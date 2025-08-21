use std::env;
use std::path::{Path, PathBuf};

#[cfg(feature = "download-lib")]
#[path = "build-utils/downloader.rs"]
mod downloader;

#[path = "build-utils/patch-linux.rs"]
mod patch_linux;
#[path = "build-utils/patch-macos.rs"]
mod patch_macos;
#[path = "build-utils/patch-windows.rs"]
mod patch_windows;

fn main() {
    if env::var("DOCS_RS").is_ok() {
        // On docs.rs we don't need to link and we don't have network,
        // so we couldn't download anything if we wanted to
        return;
    }

    #[cfg(feature = "download-lib")]
    let lib_path = {
        let downloaded_path = download_lib();
        downloaded_path.join("lib")
    };

    #[cfg(not(feature = "download-lib"))]
    let lib_path = PathBuf::from(
        env::var("AIC_LIB_PATH").expect("Enable feature `download-lib` or use a local library by setting the environment variable `AIC_LIB_PATH`"),
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Use different approaches for Windows vs other platforms
    if cfg!(target_os = "windows") {
        handle_windows_linking(&lib_path);
    } else {
        let lib_name = "aic";
        let lib_name_patched = "aic_patched";

        patch_lib(&lib_path, lib_name, lib_name_patched);

        // Link with the curated library
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=static={lib_name_patched}");
    }

    // Add platform-specific system libraries
    add_platform_specific_libs();

    generate_bindings();
}

fn add_platform_specific_libs() {
    if cfg!(target_os = "macos") {
        // macOS requires CoreFoundation framework for time zone operations
        // This is needed by chrono and other crates that interact with system time
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        
        // Security framework might also be needed for some operations
        println!("cargo:rustc-link-lib=framework=Security");
    } else if cfg!(target_os = "windows") {
        // Windows system libraries that might be needed
        println!("cargo:rustc-link-lib=advapi32");
        println!("cargo:rustc-link-lib=bcrypt");
        println!("cargo:rustc-link-lib=kernel32");
        println!("cargo:rustc-link-lib=ws2_32");
    } else if cfg!(target_os = "linux") {
        // Linux system libraries
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=rt");
    }
}

#[cfg(feature = "download-lib")]
fn download_lib() -> PathBuf {
    use downloader::Downloader;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let downloader = Downloader::new(&out_dir);
    downloader.download()
}

fn patch_lib(lib_path: &Path, lib_name: &str, lib_name_patched: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let static_lib_ext = if cfg!(target_os = "windows") { ".lib" } else { ".a" };
    let static_lib = if cfg!(target_os = "windows") {
        lib_path.join(format!("{}{}", lib_name, static_lib_ext))
    } else {
        lib_path.join(format!("lib{}{}", lib_name, static_lib_ext))
    };

    let global_symbols_wildcard = "aic_*";

    if !static_lib.exists() {
        panic!("Please provide the SDK at {}", static_lib.display());
    }

    let final_lib = if cfg!(target_os = "windows") {
        out_dir.join(format!("{}{}", lib_name_patched, static_lib_ext))
    } else {
        out_dir.join(format!("lib{}{}", lib_name_patched, static_lib_ext))
    };

    if cfg!(target_os = "linux") {
        patch_linux::patch_lib(&static_lib, &out_dir, lib_name, lib_name_patched, global_symbols_wildcard, &final_lib);
    } else if cfg!(target_os = "macos") {
        patch_macos::patch_lib(&static_lib, &out_dir, lib_name, lib_name_patched, global_symbols_wildcard, &final_lib);
    } else if cfg!(target_os = "windows") {
        patch_windows::patch_lib(&static_lib, &out_dir, lib_name, lib_name_patched, global_symbols_wildcard, &final_lib);
    } else {
        panic!("Unsupported platform for library patching");
    }

    // Rerun this script if the static library changes.
    println!("cargo:rerun-if-changed={}", static_lib.display());
}

fn handle_windows_linking(lib_path: &Path) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // On Windows, look for the DLL instead of static library
    let dll_name = "aic.dll";
    let import_lib_name = "aic.lib";
    
    // Try to find the DLL in the SDK directory
    // First check if it's directly in lib_path
    let dll_path = lib_path.join(dll_name);
    let import_lib_path = lib_path.join(import_lib_name);
    
    // If not found in lib/, try the parent directory (root of extracted SDK)
    let (dll_path, import_lib_path) = if !dll_path.exists() {
        let parent_dir = lib_path.parent().unwrap_or(lib_path);
        (parent_dir.join(dll_name), parent_dir.join(import_lib_name))
    } else {
        (dll_path, import_lib_path)
    };
    
    if !dll_path.exists() {
        panic!("Could not find aic.dll at {} or in parent directory", dll_path.display());
    }
    
    println!("cargo:warning=Using DLL linking approach on Windows: {}", dll_path.display());
    
    // Check DLL dependencies to help debug runtime issues
    check_dll_dependencies(&dll_path);
    
    // Add enhanced diagnostics for Windows CI
    println!("cargo:warning=Windows DLL diagnostics:");
    println!("cargo:warning=- aic.dll copied to multiple locations");
    println!("cargo:warning=- VCRUNTIME140.dll should be available in System32");
    println!("cargo:warning=- If STATUS_DLL_NOT_FOUND persists, check Universal CRT availability");
    
    // Copy the DLL to the output directory so it can be found at runtime
    let out_dll_path = out_dir.join(dll_name);
    std::fs::copy(&dll_path, &out_dll_path)
        .expect("Failed to copy DLL to output directory");
    
    // Always generate our own import library from the DLL to avoid symbol conflicts
    // The existing aic.lib is likely a static library with conflicting symbols
    if import_lib_path.exists() {
        println!("cargo:warning=Found existing aic.lib but generating new import library from DLL to avoid symbol conflicts");
    }
    
    // Try to generate import library using available tools
    if try_generate_import_library(&dll_path, &out_dir) {
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=dylib=aic");
    } else if try_generate_import_library_llvm(&dll_path, &out_dir) {
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=dylib=aic");
    } else {
        // Last resort: create a minimal import library manually or avoid the conflicting static lib
        println!("cargo:warning=Couldn't generate import library with any available tools, trying manual approach");
        if try_manual_import_library(&dll_path, &out_dir) {
            println!("cargo:rustc-link-search=native={}", out_dir.display());
            println!("cargo:rustc-link-lib=dylib=aic");
        } else {
            // Final fallback: copy DLL to clean location and try direct linking
            let clean_dll_dir = out_dir.join("dll_clean");
            std::fs::create_dir_all(&clean_dll_dir).expect("Failed to create clean DLL directory");
            let clean_dll_path = clean_dll_dir.join("aic.dll");
            std::fs::copy(&dll_path, &clean_dll_path).expect("Failed to copy DLL to clean location");
            
            println!("cargo:warning=Using direct DLL linking from clean directory");
            println!("cargo:rustc-link-search=native={}", clean_dll_dir.display());
            println!("cargo:rustc-link-lib=dylib=aic");
        }
    }
    
    // Tell cargo to rerun if the DLL changes
    println!("cargo:rerun-if-changed={}", dll_path.display());
    
    // Copy DLL to target directory for examples and tests
    copy_dll_to_target(&dll_path);
    
    // Also copy any additional DLLs that might be dependencies
    // copy_additional_dlls(&dll_path); // TODO: Fix function order issue
    
    // Also set up cargo instruction to copy DLL at runtime
    println!("cargo:rustc-env=AIC_DLL_PATH={}", dll_path.display());
    
    // Set up PATH environment variable to include DLL directory for runtime
    if let Some(dll_dir) = dll_path.parent() {
        println!("cargo:rustc-env=AIC_DLL_DIR={}", dll_dir.display());
    }
    
    // For CI environments, also try copying based on workspace structure
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    if let Some(workspace_root) = manifest_dir.parent() {
        let workspace_target = workspace_root.join("target").join(env::var("PROFILE").unwrap_or_else(|_| "debug".to_string()));
        if workspace_target.exists() || workspace_target.parent().unwrap().exists() {
            let _ = std::fs::create_dir_all(&workspace_target);
            
            // Copy to workspace target root
            let workspace_dll = workspace_target.join("aic.dll");
            if let Ok(_) = std::fs::copy(&dll_path, &workspace_dll) {
                println!("cargo:warning=Copied DLL to workspace target: {}", workspace_dll.display());
            }
            
            // Copy to workspace examples directory
            let workspace_examples = workspace_target.join("examples");
            let _ = std::fs::create_dir_all(&workspace_examples);
            let workspace_examples_dll = workspace_examples.join("aic.dll");
            if let Ok(_) = std::fs::copy(&dll_path, &workspace_examples_dll) {
                println!("cargo:warning=Copied DLL to workspace examples: {}", workspace_examples_dll.display());
            }
        }
    }
}

fn try_generate_import_library(dll_path: &Path, out_dir: &Path) -> bool {
    use std::process::Command;
    
    // Try to generate import library using lib.exe
    let import_lib_path = out_dir.join("aic.lib");
    let def_file_path = out_dir.join("aic.def");
    
    // Clean up any existing files first
    let _ = std::fs::remove_file(&import_lib_path);
    let _ = std::fs::remove_file(&def_file_path);
    
    println!("cargo:warning=Attempting to generate import library from DLL using Microsoft tools");
    
    // First, try to generate a .def file from the DLL
    match Command::new("dumpbin")
        .arg("/EXPORTS")
        .arg(dll_path)
        .output() 
    {
        Ok(output) if output.status.success() => {
            let exports_output = String::from_utf8_lossy(&output.stdout);
            println!("cargo:warning=Successfully extracted exports from DLL");
            
            match create_def_file(&exports_output, &def_file_path) {
                Ok(export_count) => {
                    if export_count == 0 {
                        println!("cargo:warning=No aic_* exports found in DLL");
                        return false;
                    }
                    println!("cargo:warning=Created .def file with {} exports", export_count);
                    
                    // Now generate the import library
                    match Command::new("lib")
                        .arg(format!("/DEF:{}", def_file_path.display()))
                        .arg(format!("/OUT:{}", import_lib_path.display()))
                        .arg("/MACHINE:X64")
                        .output()
                    {
                        Ok(lib_output) if lib_output.status.success() => {
                            println!("cargo:warning=Successfully generated import library: {}", import_lib_path.display());
                            let _ = std::fs::remove_file(&def_file_path); // Cleanup .def file
                            return true;
                        }
                        Ok(lib_output) => {
                            let stderr = String::from_utf8_lossy(&lib_output.stderr);
                            println!("cargo:warning=lib.exe failed: {}", stderr);
                        }
                        Err(e) => {
                            println!("cargo:warning=Failed to execute lib.exe: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("cargo:warning=Failed to create .def file: {}", e);
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("cargo:warning=dumpbin failed: {}", stderr);
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute dumpbin: {}", e);
        }
    }
    
    // Cleanup on failure
    let _ = std::fs::remove_file(&def_file_path);
    let _ = std::fs::remove_file(&import_lib_path);
    false
}

fn create_def_file(dumpbin_output: &str, def_file_path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;
    
    let mut def_content = String::from("EXPORTS\n");
    let mut in_exports_section = false;
    let mut export_count = 0;
    
    for line in dumpbin_output.lines() {
        let line = line.trim();
        
        if line.contains("ordinal hint RVA      name") || line.contains("ordinal  hint RVA      name") {
            in_exports_section = true;
            continue;
        }
        
        if in_exports_section {
            if line.is_empty() || line.starts_with("Summary") {
                break;
            }
            
            // Parse the dumpbin export line format
            // Example: "    1    0 00001234 function_name"
            // Or sometimes: "    1          00001234 function_name" (no hint column)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                // The function name is the last part
                let function_name = parts.last().unwrap();
                // Only include functions that start with "aic" to avoid system symbols
                if function_name.starts_with("aic") {
                    def_content.push_str(&format!("    {}\n", function_name));
                    export_count += 1;
                }
            }
        }
    }
    
    if export_count > 0 {
        let mut file = File::create(def_file_path)?;
        file.write_all(def_content.as_bytes())?;
    }
    
    Ok(export_count)
}

fn try_generate_import_library_llvm(dll_path: &Path, out_dir: &Path) -> bool {
    use std::process::Command;
    
    // Check if LLVM tools are available
    if Command::new("llvm-dlltool").arg("--help").output().is_err() {
        return false;
    }
    
    println!("cargo:warning=Attempting to generate import library using LLVM tools");
    
    let import_lib_path = out_dir.join("aic.lib");
    let def_file_path = out_dir.join("aic.def");
    
    // Clean up any existing files first
    let _ = std::fs::remove_file(&import_lib_path);
    let _ = std::fs::remove_file(&def_file_path);
    
    // First try to extract exports using llvm-objdump
    match Command::new("llvm-objdump")
        .arg("--exports")
        .arg(dll_path)
        .output() 
    {
        Ok(output) if output.status.success() => {
            let exports_output = String::from_utf8_lossy(&output.stdout);
            
            match create_def_file_from_llvm(&exports_output, &def_file_path) {
                Ok(export_count) if export_count > 0 => {
                    println!("cargo:warning=Created .def file with {} exports using LLVM", export_count);
                    
                    // Generate import library using llvm-dlltool
                    match Command::new("llvm-dlltool")
                        .arg("-d").arg(&def_file_path)
                        .arg("-l").arg(&import_lib_path)
                        .arg("-m").arg("i386:x86-64")
                        .output()
                    {
                        Ok(dll_output) if dll_output.status.success() => {
                            println!("cargo:warning=Successfully generated import library using LLVM: {}", import_lib_path.display());
                            let _ = std::fs::remove_file(&def_file_path);
                            return true;
                        }
                        Ok(dll_output) => {
                            let stderr = String::from_utf8_lossy(&dll_output.stderr);
                            println!("cargo:warning=llvm-dlltool failed: {}", stderr);
                        }
                        Err(e) => {
                            println!("cargo:warning=Failed to execute llvm-dlltool: {}", e);
                        }
                    }
                }
                _ => {
                    println!("cargo:warning=No aic_* exports found using LLVM tools");
                }
            }
        }
        _ => {
            println!("cargo:warning=llvm-objdump failed or not available");
        }
    }
    
    // Cleanup on failure
    let _ = std::fs::remove_file(&def_file_path);
    let _ = std::fs::remove_file(&import_lib_path);
    false
}

fn create_def_file_from_llvm(llvm_output: &str, def_file_path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;
    
    let mut def_content = String::from("EXPORTS\n");
    let mut export_count = 0;
    
    // Parse llvm-objdump export output
    for line in llvm_output.lines() {
        let line = line.trim();
        
        // Look for export entries (format varies)
        if line.contains("aic") {
            // Try to extract function name
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts {
                if part.starts_with("aic") {
                    def_content.push_str(&format!("    {}\n", part));
                    export_count += 1;
                    break;
                }
            }
        }
    }
    
    if export_count > 0 {
        let mut file = File::create(def_file_path)?;
        file.write_all(def_content.as_bytes())?;
    }
    
    Ok(export_count)
}

fn try_manual_import_library(_dll_path: &Path, out_dir: &Path) -> bool {
    println!("cargo:warning=Attempting to create minimal import library manually");
    
    let import_lib_path = out_dir.join("aic.lib");
    let def_file_path = out_dir.join("aic.def");
    
    // Clean up any existing files first
    let _ = std::fs::remove_file(&import_lib_path);
    let _ = std::fs::remove_file(&def_file_path);
    
    // Try to create a minimal .def file with common aic functions
    // This is a fallback approach when we can't inspect the DLL
    match create_minimal_def_file(&def_file_path) {
        Ok(_) => {
            println!("cargo:warning=Created minimal .def file");
            
            // Try any available tool to create import library
            if try_create_lib_with_any_tool(&def_file_path, &import_lib_path) {
                let _ = std::fs::remove_file(&def_file_path);
                return true;
            }
        }
        Err(e) => {
            println!("cargo:warning=Failed to create minimal .def file: {}", e);
        }
    }
    
    // Cleanup on failure
    let _ = std::fs::remove_file(&def_file_path);
    let _ = std::fs::remove_file(&import_lib_path);
    false
}

fn create_minimal_def_file(def_file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;
    
    // Create a .def file with actual function names from the AIC SDK header
    let def_content = r#"EXPORTS
    aic_model_create
    aic_model_destroy
    aic_model_initialize
    aic_model_reset
    aic_model_process_planar
    aic_model_process_interleaved
    aic_model_set_parameter
    aic_model_get_parameter
    aic_get_output_delay
    aic_get_optimal_sample_rate
    aic_get_optimal_num_frames
    aic_get_sdk_version
"#;
    
    let mut file = File::create(def_file_path)?;
    file.write_all(def_content.as_bytes())?;
    Ok(())
}

fn try_create_lib_with_any_tool(def_file_path: &Path, import_lib_path: &Path) -> bool {
    use std::process::Command;
    
    // Try lib.exe (Microsoft)
    if let Ok(output) = Command::new("lib")
        .arg(format!("/DEF:{}", def_file_path.display()))
        .arg(format!("/OUT:{}", import_lib_path.display()))
        .arg("/MACHINE:X64")
        .output()
    {
        if output.status.success() {
            println!("cargo:warning=Created import library with lib.exe");
            return true;
        }
    }
    
    // Try llvm-dlltool
    if let Ok(output) = Command::new("llvm-dlltool")
        .arg("-d").arg(def_file_path)
        .arg("-l").arg(import_lib_path)
        .arg("-m").arg("i386:x86-64")
        .output()
    {
        if output.status.success() {
            println!("cargo:warning=Created import library with llvm-dlltool");
            return true;
        }
    }
    
    // Try dlltool (MinGW)
    if let Ok(output) = Command::new("dlltool")
        .arg("-d").arg(def_file_path)
        .arg("-l").arg(import_lib_path)
        .arg("-m").arg("i386:x86-64")
        .output()
    {
        if output.status.success() {
            println!("cargo:warning=Created import library with dlltool");
            return true;
        }
    }
    
    false
}

// Unused helper functions removed

fn check_dll_dependencies(dll_path: &Path) {
    use std::process::Command;
    
    // Try to check DLL dependencies using dumpbin if available
    if let Ok(output) = Command::new("dumpbin")
        .arg("/DEPENDENTS")
        .arg(dll_path)
        .output()
    {
        if output.status.success() {
            let deps_output = String::from_utf8_lossy(&output.stdout);
            println!("cargo:warning=DLL dependencies found:");
            for line in deps_output.lines() {
                if line.trim().ends_with(".dll") {
                    println!("cargo:warning=  -> {}", line.trim());
                }
            }
        }
    } else if let Ok(output) = Command::new("objdump")
        .arg("-p")
        .arg(dll_path)
        .output()
    {
        if output.status.success() {
            let deps_output = String::from_utf8_lossy(&output.stdout);
            println!("cargo:warning=DLL dependencies (objdump):");
            for line in deps_output.lines() {
                if line.contains("DLL Name:") {
                    println!("cargo:warning=  -> {}", line.trim());
                }
            }
        }
    }
    
    // Also check if the DLL exists and is readable
    match std::fs::metadata(dll_path) {
        Ok(metadata) => {
            println!("cargo:warning=DLL file size: {} bytes", metadata.len());
        }
        Err(e) => {
            println!("cargo:warning=Error reading DLL metadata: {}", e);
        }
    }
}

fn copy_dll_to_target(dll_path: &Path) {
    // Copy DLL to multiple locations where the executable might look for it
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    
    // Try to get the correct target directory
    let target_dirs = if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        vec![PathBuf::from(target_dir)]
    } else {
        // Try both workspace root and current directory
        vec![
            PathBuf::from("../../target"),  // From aic-sdk-sys directory to workspace root
            PathBuf::from("../target"),     // From aic-sdk-sys directory to parent
            PathBuf::from("target"),        // Current directory
        ]
    };
    
    for target_path in target_dirs {
        if target_path.exists() || target_path.parent().map(|p| p.exists()).unwrap_or(false) {
            // Create target directory if it doesn't exist
            let profile_dir = target_path.join(&profile);
            let _ = std::fs::create_dir_all(&profile_dir);
            
            let dll_target_path = profile_dir.join("aic.dll");
            if let Ok(_) = std::fs::copy(dll_path, &dll_target_path) {
                println!("cargo:warning=Copied DLL to target directory: {}", dll_target_path.display());
                
                // Also copy to examples subdirectory for examples
                let examples_dir = profile_dir.join("examples");
                let _ = std::fs::create_dir_all(&examples_dir);
                let examples_dll_path = examples_dir.join("aic.dll");
                if let Ok(_) = std::fs::copy(dll_path, &examples_dll_path) {
                    println!("cargo:warning=Copied DLL to examples directory: {}", examples_dll_path.display());
                }
                
                // Also copy to deps subdirectory 
                let deps_dir = profile_dir.join("deps");
                let _ = std::fs::create_dir_all(&deps_dir);
                let deps_dll_path = deps_dir.join("aic.dll");
                let _ = std::fs::copy(dll_path, &deps_dll_path);
                
                break; // Successfully copied, no need to try other paths
            }
        }
    }
    
    // Additional fallback: copy to the output directory itself for build scripts to find
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_dll_path = out_dir.join("aic.dll");
    let _ = std::fs::copy(dll_path, &out_dll_path);
    
    // Try to determine the actual target directory from OUT_DIR
    // OUT_DIR is typically something like: target/debug/build/crate-name-hash/out
    if let Some(build_dir) = out_dir.parent() {
        if let Some(crate_build_dir) = build_dir.parent() {
            if let Some(build_root) = crate_build_dir.parent() {
                if let Some(target_dir) = build_root.parent() {
                    // Now target_dir should be the actual target directory
                    let profile_dir = target_dir.join(&profile);
                    
                    // Copy to main target directory
                    let main_dll_path = profile_dir.join("aic.dll");
                    if let Ok(_) = std::fs::copy(dll_path, &main_dll_path) {
                        println!("cargo:warning=Copied DLL to main target: {}", main_dll_path.display());
                    }
                    
                    // Copy to examples directory 
                    let examples_dir = profile_dir.join("examples");
                    let _ = std::fs::create_dir_all(&examples_dir);
                    let examples_dll_path = examples_dir.join("aic.dll");
                    if let Ok(_) = std::fs::copy(dll_path, &examples_dll_path) {
                        println!("cargo:warning=Copied DLL to examples (from OUT_DIR): {}", examples_dll_path.display());
                    }
                    
                    // Copy to deps directory
                    let deps_dir = profile_dir.join("deps");
                    let _ = std::fs::create_dir_all(&deps_dir);
                    let deps_dll_path = deps_dir.join("aic.dll");
                    let _ = std::fs::copy(dll_path, &deps_dll_path);
                }
            }
        }
    }
}

// Duplicate functions removed - using the original definitions above

fn generate_bindings() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let header_path = manifest_dir.join("include").join("aic.h");

    // Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate bindings for.
        .header(header_path.to_str().unwrap())
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Generate constified enums to avoid name repetition
        .constified_enum_module("AicErrorCode")
        .constified_enum_module("AicParameter")
        .constified_enum_module("AicModelType")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Tell cargo to rerun the build script if the library or header changes.
    println!("cargo:rerun-if-changed={}", header_path.display());
}
