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
    // Check if required tools are available
    if !is_command_available("llvm-ar") || !is_command_available("llvm-objcopy") || !is_command_available("llvm-nm") {
        return false;
    }
    
    println!("cargo:warning=Using LLVM tools for symbol filtering on Windows");
    
    // Create working directory for object extraction
    let work_dir = out_dir.join(format!("{}_llvm_objs", lib_name_patched));
    if work_dir.exists() {
        let _ = fs::remove_dir_all(&work_dir);
    }
    if let Err(e) = fs::create_dir(&work_dir) {
        println!("cargo:warning=Failed to create working directory: {}", e);
        return false;
    }
    
    // Extract all object files from the library using llvm-ar
    let extract_output = Command::new("llvm-ar")
        .arg("x")
        .arg(static_lib)
        .current_dir(&work_dir)
        .output();
    
    let extract_success = match extract_output {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            println!("cargo:warning=llvm-ar extraction failed: {}", String::from_utf8_lossy(&output.stderr));
            false
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute llvm-ar: {}", e);
            false
        }
    };
    
    if !extract_success {
        let _ = fs::remove_dir_all(&work_dir);
        return false;
    }
    
    // Process each object file
    let mut filtered_objects = Vec::new();
    let prefix = global_symbols_wildcard.trim_end_matches('*');
    
    let entries = match fs::read_dir(&work_dir) {
        Ok(entries) => entries,
        Err(e) => {
            println!("cargo:warning=Failed to read working directory: {}", e);
            let _ = fs::remove_dir_all(&work_dir);
            return false;
        }
    };
    
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("o") {
            continue;
        }
        
        // Check if this object contains symbols we want to keep
        let should_keep = match check_object_symbols_llvm(&path, prefix) {
            Ok(has_symbols) => has_symbols,
            Err(_) => {
                // If we can't analyze it, keep it to be safe
                true
            }
        };
        
        if should_keep {
            // Filter symbols in this object file
            let filtered_obj = work_dir.join(format!("filtered_{}", path.file_name().unwrap().to_string_lossy()));
            if filter_object_symbols_llvm(&path, &filtered_obj, prefix).is_ok() {
                filtered_objects.push(filtered_obj);
            } else {
                // If filtering fails, use the original
                filtered_objects.push(path);
            }
        }
    }
    
    if filtered_objects.is_empty() {
        println!("cargo:warning=No objects contain desired symbols, using library as-is");
        fs::copy(static_lib, final_lib).expect("Failed to copy library file");
        let _ = fs::remove_dir_all(&work_dir);
        return true;
    }
    
    // Create new library with filtered objects using llvm-ar
    let mut cmd = Command::new("llvm-ar");
    cmd.arg("rcs").arg(final_lib);
    for obj in &filtered_objects {
        cmd.arg(obj);
    }
    
    let ar_success = match cmd.output() {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            println!("cargo:warning=llvm-ar library creation failed: {}", String::from_utf8_lossy(&output.stderr));
            false
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute llvm-ar for library creation: {}", e);
            false
        }
    };
    
    // Cleanup
    let _ = fs::remove_dir_all(&work_dir);
    
    ar_success
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
    
    // Extract all object files from the library using lib.exe
    // Note: lib.exe /EXTRACT extracts all members, we need to do this differently
    let list_output = Command::new("lib")
        .arg("/LIST")
        .arg(static_lib)
        .output();
    
    let object_names = match list_output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty() && line.ends_with(".obj"))
                .collect::<Vec<_>>()
        }
        Ok(output) => {
            println!("cargo:warning=lib /LIST failed: {}", String::from_utf8_lossy(&output.stderr));
            let _ = fs::remove_dir_all(&work_dir);
            return false;
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute lib /LIST: {}", e);
            let _ = fs::remove_dir_all(&work_dir);
            return false;
        }
    };
    
    if object_names.is_empty() {
        println!("cargo:warning=No object files found in library");
        let _ = fs::remove_dir_all(&work_dir);
        return false;
    }
    
    // Extract each object file individually
    let mut extract_success = true;
    for obj_name in &object_names {
        let extract_status = Command::new("lib")
            .arg(format!("/EXTRACT:{}", obj_name))
            .arg(format!("/OUT:{}", work_dir.join(obj_name).display()))
            .arg(static_lib)
            .status();
        
        if let Ok(status) = extract_status {
            if !status.success() {
                println!("cargo:warning=Failed to extract {}", obj_name);
                extract_success = false;
                break;
            }
        } else {
            println!("cargo:warning=Failed to execute lib /EXTRACT for {}", obj_name);
            extract_success = false;
            break;
        }
    }
    
    let success = if extract_success {
        // Analyze objects and keep only those with desired symbols
        if let Ok(filtered_objects) = filter_objects_msvc(&work_dir, global_symbols_wildcard) {
            // Create new library with filtered objects
            create_filtered_library_msvc(&filtered_objects, final_lib).is_ok()
        } else {
            false
        }
    } else {
        false
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



fn check_object_symbols_llvm(obj_path: &Path, prefix: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("llvm-nm")
        .arg("--defined-only")
        .arg("--extern-only")
        .arg(obj_path)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("llvm-nm failed on object: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    for line in stdout.lines() {
        if let Some(symbol) = parse_nm_symbol_windows(line) {
            if symbol.starts_with(prefix) {
                return Ok(true);
            }
        }
    }
    
    Ok(false)
}

fn filter_object_symbols_llvm(input_obj: &Path, output_obj: &Path, prefix: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Get all symbols from this object
    let output = Command::new("llvm-nm")
        .arg("--defined-only")
        .arg("--extern-only")
        .arg(input_obj)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("llvm-nm failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut symbols_to_strip = Vec::new();
    
    for line in stdout.lines() {
        if let Some(symbol) = parse_nm_symbol_windows(line) {
            // Strip symbols that don't start with our prefix
            if !symbol.starts_with(prefix) {
                symbols_to_strip.push(symbol);
            }
        }
    }
    
    if symbols_to_strip.is_empty() {
        // No symbols to strip, just copy the file
        fs::copy(input_obj, output_obj)?;
        return Ok(());
    }
    
    // Create symbols file for stripping
    let symbols_file = input_obj.with_extension("strip_symbols");
    let symbols_content = symbols_to_strip.join("\n");
    fs::write(&symbols_file, symbols_content)?;
    
    // Use llvm-objcopy to strip unwanted symbols
    let objcopy_output = Command::new("llvm-objcopy")
        .arg(format!("--strip-symbols={}", symbols_file.display()))
        .arg(input_obj)
        .arg(output_obj)
        .output()?;
    
    // Cleanup
    let _ = fs::remove_file(&symbols_file);
    
    if !objcopy_output.status.success() {
        let stderr = String::from_utf8_lossy(&objcopy_output.stderr);
        return Err(format!("llvm-objcopy failed on object: {}", stderr).into());
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
