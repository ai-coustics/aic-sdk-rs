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
        // Last resort: try to use the DLL directly (may not work on all systems)
        println!("cargo:warning=Couldn't generate import library with any available tools, trying direct DLL linking");
        println!("cargo:rustc-link-search=native={}", dll_path.parent().unwrap().display());
        println!("cargo:rustc-link-lib=dylib=aic");
    }
    
    // Tell cargo to rerun if the DLL changes
    println!("cargo:rerun-if-changed={}", dll_path.display());
    
    // Copy DLL to target directory for examples and tests
    copy_dll_to_target(&dll_path);
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

fn copy_dll_to_target(dll_path: &Path) {
    // Copy DLL to target directory so examples and tests can find it
    if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        let target_path = PathBuf::from(target_dir);
        let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        let dll_target_path = target_path.join(profile).join("aic.dll");
        
        let _ = std::fs::copy(dll_path, &dll_target_path);
        println!("cargo:warning=Copied DLL to target directory: {}", dll_target_path.display());
    } else {
        // Fallback: try relative path
        let target_path = PathBuf::from("../target");
        if target_path.exists() {
            let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
            let dll_target_path = target_path.join(profile).join("aic.dll");
            let _ = std::fs::copy(dll_path, &dll_target_path);
            println!("cargo:warning=Copied DLL to target directory: {}", dll_target_path.display());
        }
    }
}

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
