use std::path::Path;
use std::process::Command;
use std::fs;

pub fn patch_lib(static_lib: &Path, out_dir: &Path, lib_name: &str, lib_name_patched: &str, global_symbols_wildcard: &str, final_lib: &Path) {
    // Try different approaches for Windows symbol filtering
    // 1. LLVM tools approach (preferred, cross-platform)
    // 2. Microsoft toolchain approach
    // 3. Fallback: copy as-is
    
    if try_llvm_approach(static_lib, out_dir, lib_name, lib_name_patched, global_symbols_wildcard, final_lib) {
        return;
    }
    
    if try_msvc_approach(static_lib, out_dir, lib_name, lib_name_patched, global_symbols_wildcard, final_lib) {
        return;
    }
    
    // Fallback: just copy the library
    println!("cargo:warning=No suitable tools found for symbol filtering on Windows, using library as-is");
    fs::copy(static_lib, final_lib)
        .expect("Failed to copy library file");
}

fn try_llvm_approach(static_lib: &Path, out_dir: &Path, _lib_name: &str, lib_name_patched: &str, global_symbols_wildcard: &str, final_lib: &Path) -> bool {
    // Check if LLVM tools are available
    if !is_command_available("llvm-nm") || !is_command_available("llvm-objcopy") {
        return false;
    }
    
    println!("cargo:warning=Using LLVM tools for symbol filtering on Windows");
    
    // Create intermediate file
    let intermediate_lib = out_dir.join(format!("{}_intermediate.lib", lib_name_patched));
    
    // First, copy the original library
    if let Err(e) = fs::copy(static_lib, &intermediate_lib) {
        println!("cargo:warning=Failed to copy library for LLVM processing: {}", e);
        return false;
    }
    
    // Get symbols from the library
    let symbols = match get_symbols_llvm(&intermediate_lib, global_symbols_wildcard) {
        Ok(symbols) => symbols,
        Err(e) => {
            println!("cargo:warning=Failed to extract symbols with llvm-nm: {}", e);
            let _ = fs::remove_file(&intermediate_lib);
            return false;
        }
    };
    
    if symbols.is_empty() {
        println!("cargo:warning=No matching symbols found, using library as-is");
        fs::rename(&intermediate_lib, final_lib)
            .expect("Failed to rename library file");
        return true;
    }
    
    // Filter symbols using llvm-objcopy
    if let Err(e) = filter_symbols_llvm(&intermediate_lib, final_lib, &symbols) {
        println!("cargo:warning=Failed to filter symbols with llvm-objcopy: {}", e);
        let _ = fs::remove_file(&intermediate_lib);
        return false;
    }
    
    // Cleanup
    let _ = fs::remove_file(&intermediate_lib);
    true
}

fn try_msvc_approach(static_lib: &Path, out_dir: &Path, _lib_name: &str, lib_name_patched: &str, global_symbols_wildcard: &str, final_lib: &Path) -> bool {
    // Check if Microsoft tools are available
    if !is_command_available("lib") || !is_command_available("dumpbin") {
        return false;
    }
    
    println!("cargo:warning=Using Microsoft tools for symbol filtering on Windows");
    
    // Create working directory for object extraction
    let work_dir = out_dir.join(format!("{}_objs", lib_name_patched));
    if work_dir.exists() {
        let _ = fs::remove_dir_all(&work_dir);
    }
    fs::create_dir(&work_dir).expect("Failed to create working directory");
    
    // Extract all object files from the library
    let extract_status = Command::new("lib")
        .arg("/EXTRACT")
        .arg(format!("/OUT:{}", work_dir.display()))
        .arg(static_lib)
        .status();
    
    let success = match extract_status {
        Ok(status) if status.success() => {
            // Analyze objects and keep only those with desired symbols
            if let Ok(filtered_objects) = filter_objects_msvc(&work_dir, global_symbols_wildcard) {
                // Create new library with filtered objects
                create_filtered_library_msvc(&filtered_objects, final_lib).is_ok()
            } else {
                false
            }
        }
        _ => false,
    };
    
    // Cleanup
    let _ = fs::remove_dir_all(&work_dir);
    
    if !success {
        println!("cargo:warning=Microsoft toolchain approach failed");
    }
    
    success
}

fn is_command_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--help")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false) ||
    Command::new(cmd)
        .arg("/?")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn get_symbols_llvm(lib_path: &Path, wildcard: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let output = Command::new("llvm-nm")
        .arg("--defined-only")
        .arg("--extern-only")
        .arg(lib_path)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("llvm-nm failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut symbols = Vec::new();
    
    // Convert wildcard to prefix (aic_* becomes aic_)
    let prefix = wildcard.trim_end_matches('*');
    
    for line in stdout.lines() {
        if let Some(symbol) = parse_nm_symbol_windows(line) {
            if symbol.starts_with(prefix) {
                symbols.push(symbol);
            }
        }
    }
    
    Ok(symbols)
}

fn parse_nm_symbol_windows(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    // llvm-nm output format: [address] [type] [symbol]
    if parts.len() >= 3 {
        let symbol_type = parts[1];
        let symbol_name = parts[2];
        
        // Keep global symbols (uppercase types)
        if symbol_type.chars().any(|c| c.is_uppercase()) {
            Some(symbol_name.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

fn filter_symbols_llvm(input_lib: &Path, output_lib: &Path, symbols_to_keep: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Create a symbols file
    let symbols_file = input_lib.with_extension("symbols");
    let symbols_content = symbols_to_keep.join("\n");
    fs::write(&symbols_file, symbols_content)?;
    
    // Use llvm-objcopy to keep only specified symbols
    let status = Command::new("llvm-objcopy")
        .arg(format!("--keep-global-symbols={}", symbols_file.display()))
        .arg(input_lib)
        .arg(output_lib)
        .status()?;
    
    // Cleanup
    let _ = fs::remove_file(&symbols_file);
    
    if !status.success() {
        return Err("llvm-objcopy failed".into());
    }
    
    Ok(())
}

fn filter_objects_msvc(work_dir: &Path, wildcard: &str) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    let mut filtered_objects = Vec::new();
    let prefix = wildcard.trim_end_matches('*');
    
    // Scan all .obj files in the working directory
    for entry in fs::read_dir(work_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("obj") {
            // Use dumpbin to check symbols in this object
            let output = Command::new("dumpbin")
                .arg("/SYMBOLS")
                .arg(&path)
                .output()?;
            
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                
                // Check if this object contains symbols we want to keep
                for line in stdout.lines() {
                    if line.contains(prefix) {
                        filtered_objects.push(path.clone());
                        break;
                    }
                }
            }
        }
    }
    
    Ok(filtered_objects)
}

fn create_filtered_library_msvc(objects: &[std::path::PathBuf], output_lib: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if objects.is_empty() {
        return Err("No objects to include in library".into());
    }
    
    let mut cmd = Command::new("lib");
    cmd.arg(format!("/OUT:{}", output_lib.display()));
    
    for obj in objects {
        cmd.arg(obj);
    }
    
    let status = cmd.status()?;
    
    if !status.success() {
        return Err("lib command failed to create filtered library".into());
    }
    
    Ok(())
}
