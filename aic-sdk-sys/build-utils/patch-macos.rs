use std::env;
use std::path::Path;
use std::process::Command;
use std::fs;

pub fn patch_lib(static_lib: &Path, out_dir: &Path, lib_name: &str, lib_name_patched: &str, _global_symbols_wildcard: &str, final_lib: &Path) {
    // macOS approach: Use ld -r to create intermediate object, then create filtered library
    
    // Get target architecture for macOS linker
    let target_arch = get_macos_arch();
    
    // Create intermediate object by linking all objects from the archive
    let intermediate_obj = out_dir.join(format!("lib{}_intermediate.o", lib_name));
    
    // Use ld -r with -all_load (macOS equivalent of --whole-archive)
    let ld_status = Command::new("ld")
        .arg("-arch")
        .arg(&target_arch)
        .arg("-r")
        .arg("-o")
        .arg(&intermediate_obj)
        .arg("-all_load")
        .arg(&static_lib)
        .status()
        .expect("Failed to execute ld command on macOS");

    if !ld_status.success() {
        panic!("ld -r command failed for {} on macOS", static_lib.display());
    }

    // Verify the intermediate object was created
    if !intermediate_obj.exists() {
        panic!("Intermediate object file was not created: {}", intermediate_obj.display());
    }

    // Create symbols file listing symbols to keep
    let symbols_file = out_dir.join("symbols_to_keep.txt");
    match create_macos_symbols_file(&intermediate_obj, &symbols_file) {
        Ok(symbol_count) => {
            if symbol_count == 0 {
                println!("cargo:warning=No aic_* symbols found in macOS library, using library as-is");
                // Just copy the original library since no filtering is needed
                fs::copy(static_lib, final_lib)
                    .expect("Failed to copy library file");
                let _ = fs::remove_file(&intermediate_obj);
                return;
            }
        }
        Err(e) => {
            println!("cargo:warning=Symbol analysis failed on macOS: {}", e);
            println!("cargo:warning=Using library as-is without symbol filtering");
            // Fallback: just copy the original library
            fs::copy(static_lib, final_lib)
                .expect("Failed to copy library file");
            let _ = fs::remove_file(&intermediate_obj);
            return;
        }
    }

    // Create the final filtered object
    let final_obj = out_dir.join(format!("lib{}_filtered.o", lib_name_patched));
    
    // Use ld to create a new object with only the symbols we want
    let ld_filter_status = Command::new("ld")
        .arg("-arch")
        .arg(&target_arch)
        .arg("-r")
        .arg("-o")
        .arg(&final_obj)
        .arg("-exported_symbols_list")
        .arg(&symbols_file)
        .arg(&intermediate_obj)
        .status()
        .expect("Failed to execute ld filtering command on macOS");

    if !ld_filter_status.success() {
        panic!("ld filtering command failed for {} on macOS", intermediate_obj.display());
    }

    // Create the final archive
    let ar_status = Command::new("ar")
        .arg("rcs")
        .arg(&final_lib)
        .arg(&final_obj)
        .status()
        .expect("Failed to execute ar command on macOS");

    if !ar_status.success() {
        panic!("ar command failed for {} on macOS", final_obj.display());
    }

    // Cleanup temporary files
    let _ = fs::remove_file(&intermediate_obj);
    let _ = fs::remove_file(&final_obj);
    let _ = fs::remove_file(&symbols_file);
}

fn get_macos_arch() -> String {
    // Get the target architecture from Rust's build environment
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("CARGO_CFG_TARGET_ARCH not set");
    
    // Convert Rust architecture names to macOS ld architecture names
    match target_arch.as_str() {
        "aarch64" => "arm64".to_string(),
        "x86_64" => "x86_64".to_string(),
        arch => {
            println!("cargo:warning=Unknown target architecture for macOS: {}", arch);
            arch.to_string()
        }
    }
}

fn create_macos_symbols_file(obj_file: &Path, symbols_file: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    // First, let's try to get basic file info to debug
    let file_info = Command::new("file")
        .arg(&obj_file)
        .output();
    
    if let Ok(info) = file_info {
        let info_str = String::from_utf8_lossy(&info.stdout);
        println!("cargo:warning=Object file info: {}", info_str.trim());
    }

    // Try different nm command variations for macOS
    let nm_variations = [
        vec!["-g", "-defined-only"],  // Global and defined only
        vec!["-g"],                   // Just global
        vec!["-a"],                   // All symbols
        vec!["--defined-only"],       // GNU-style flag
    ];

    let mut nm_output = None;
    let mut last_error = String::new();

    for args in &nm_variations {
        let mut cmd = Command::new("nm");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(&obj_file);

        match cmd.output() {
            Ok(output) if output.status.success() => {
                nm_output = Some(output);
                break;
            }
            Ok(output) => {
                last_error = format!("nm failed with args {:?}: {}", args, String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                last_error = format!("nm command error with args {:?}: {}", args, e);
            }
        }
    }

    let nm_output = nm_output.ok_or_else(|| format!("All nm command variations failed. Last error: {}", last_error))?;

    let nm_stdout = String::from_utf8_lossy(&nm_output.stdout);
    let mut symbols_to_keep = Vec::new();
    
    // Parse nm output and collect aic_* symbols
    for line in nm_stdout.lines() {
        if let Some(symbol) = parse_nm_symbol_macos(line) {
            // Only keep symbols that start with "aic_" or "_aic_" (macOS prefixes with underscore)
            if symbol.starts_with("aic_") || symbol.starts_with("_aic_") {
                symbols_to_keep.push(symbol);
            }
        }
    }

    if symbols_to_keep.is_empty() {
        // Debug: Let's see what symbols are actually there
        println!("cargo:warning=No aic symbols found. First 10 lines of nm output:");
        for (i, line) in nm_stdout.lines().take(10).enumerate() {
            println!("cargo:warning=  {}: {}", i, line);
        }
        return Ok(0);
    }

    // Write symbols to file (one per line)  
    let symbols_content = symbols_to_keep.join("\n");
    fs::write(symbols_file, symbols_content)
        .map_err(|e| format!("Failed to write symbols file: {}", e))?;
    
    Ok(symbols_to_keep.len())
}

// Helper function to parse nm output on macOS and extract symbol names
fn parse_nm_symbol_macos(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    // macOS nm output can have different formats:
    // Format 1: [address] [type] [symbol]
    // Format 2: [type] [symbol] (for undefined symbols)
    // Format 3: [symbol] (minimal output)
    
    if parts.len() >= 3 {
        // Standard format: address, type, symbol
        let symbol_type = parts[1];
        let symbol_name = parts[2];
        
        // Check if this is a defined global symbol
        // Capital letters indicate global symbols, lowercase are local
        // Common types: T (text), D (data), B (bss), S (other section)
        if symbol_type.chars().any(|c| c.is_uppercase()) {
            Some(symbol_name.to_string())
        } else {
            None
        }
    } else if parts.len() == 2 {
        // Could be: type symbol OR address type
        let first = parts[0];
        let second = parts[1];
        
        // If first part looks like a type (single character), second is symbol
        if first.len() == 1 && first.chars().any(|c| c.is_uppercase()) {
            Some(second.to_string())
        } else {
            None
        }
    } else if parts.len() == 1 {
        // Just a symbol name (some nm outputs might be minimal)
        let symbol = parts[0];
        if !symbol.is_empty() && (symbol.starts_with("aic_") || symbol.starts_with("_aic_")) {
            Some(symbol.to_string())
        } else {
            None
        }
    } else {
        None
    }
}
