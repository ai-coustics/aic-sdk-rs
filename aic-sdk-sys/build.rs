use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

#[cfg(feature = "download-lib")]
#[path = "build-utils/downloader.rs"]
mod downloader;

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

    let lib_name = "aic";
    let lib_name_patched = "aic_patched";

    patch_lib(&lib_path, lib_name, lib_name_patched);

    // Link with the curated library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static={lib_name_patched}");

    generate_bindings();
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

    let final_lib = out_dir.join(format!("lib{}{}", lib_name_patched, static_lib_ext));

    if cfg!(target_os = "linux") {
        patch_lib_linux(&static_lib, &out_dir, lib_name, lib_name_patched, global_symbols_wildcard, &final_lib);
    } else if cfg!(target_os = "macos") {
        patch_lib_macos(&static_lib, &out_dir, lib_name, lib_name_patched, global_symbols_wildcard, &final_lib);
    } else if cfg!(target_os = "windows") {
        patch_lib_windows(&static_lib, &out_dir, lib_name, lib_name_patched, global_symbols_wildcard, &final_lib);
    } else {
        panic!("Unsupported platform for library patching");
    }

    // Rerun this script if the static library changes.
    println!("cargo:rerun-if-changed={}", static_lib.display());
}

fn patch_lib_linux(static_lib: &Path, out_dir: &Path, lib_name: &str, lib_name_patched: &str, global_symbols_wildcard: &str, final_lib: &Path) {
    // Original .o file
    let intermediate_obj = out_dir.join(format!("lib{}.o", lib_name));

    // Modified .o file
    let final_obj = out_dir.join(format!("lib{}.o", lib_name_patched));

    // Partially link
    let ld_status = Command::new("ld")
        .arg("-r")
        .arg("-o")
        .arg(&intermediate_obj)
        .arg("--whole-archive")
        .arg(&static_lib)
        .status()
        .expect("Failed to execute ld command.");

    if !ld_status.success() {
        panic!("ld -r command failed for {}", static_lib.display());
    }

    // Curate symbols (only keep specific symbols)
    let objcopy_status = Command::new("objcopy")
        .arg("--wildcard")
        .arg("--keep-global-symbol")
        .arg(global_symbols_wildcard)
        .arg(&intermediate_obj)
        .arg(&final_obj)
        .status()
        .expect("Failed to execute objcopy command.");

    if !objcopy_status.success() {
        panic!("objcopy command failed for {}", intermediate_obj.display());
    }

    // Build the archive
    let ar_status = Command::new("ar")
        .arg("rcs")
        .arg(&final_lib)
        .arg(&final_obj)
        .status()
        .expect("Failed to execute ar.");

    if !ar_status.success() {
        panic!("ar command failed for {}", final_obj.display());
    }
}

fn patch_lib_macos(static_lib: &Path, out_dir: &Path, lib_name: &str, lib_name_patched: &str, _global_symbols_wildcard: &str, final_lib: &Path) {
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

    // Create symbols file listing symbols to keep
    let symbols_file = out_dir.join("symbols_to_keep.txt");
    create_macos_symbols_file(&intermediate_obj, &symbols_file)
        .expect("Failed to create symbols file for macOS");

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

fn create_macos_symbols_file(obj_file: &Path, symbols_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Get all global symbols from the object file
    let nm_output = Command::new("nm")
        .arg("-g")  // Global symbols only
        .arg("-defined-only")  // Only defined symbols
        .arg(&obj_file)
        .output()?;

    if !nm_output.status.success() {
        return Err(format!("nm command failed for {}", obj_file.display()).into());
    }

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

    // Write symbols to file (one per line)
    let symbols_content = symbols_to_keep.join("\n");
    fs::write(symbols_file, symbols_content)
        .map_err(|e| format!("Failed to write symbols file: {}", e))?;
    
    Ok(())
}

fn patch_lib_windows(static_lib: &Path, out_dir: &Path, lib_name: &str, lib_name_patched: &str, _global_symbols_wildcard: &str, final_lib: &Path) {
    // Windows approach: try to use available tools to manipulate the library
    
    // First, try to find available tools
    let has_llvm_lib = Command::new("llvm-lib")
        .arg("--help")
        .output()
        .is_ok();
    
    let has_lib_exe = Command::new("lib")
        .arg("/HELP")
        .output()
        .is_ok();
    
    if has_llvm_lib {
        patch_lib_windows_llvm(static_lib, out_dir, lib_name, lib_name_patched, final_lib);
    } else if has_lib_exe {
        patch_lib_windows_msvc(static_lib, out_dir, lib_name, lib_name_patched, final_lib);
    } else {
        // Fallback: copy the library as-is
        println!("cargo:warning=No suitable library tools found on Windows (llvm-lib or lib.exe)");
        println!("cargo:warning=Copying library as-is, symbol filtering not applied");
        
        fs::copy(static_lib, final_lib)
            .expect("Failed to copy library for Windows");
    }
}

fn patch_lib_windows_llvm(static_lib: &Path, out_dir: &Path, lib_name: &str, _lib_name_patched: &str, final_lib: &Path) {
    // Extract objects from the library
    let extract_dir = out_dir.join(format!("{}_extracted", lib_name));
    fs::create_dir_all(&extract_dir).expect("Failed to create extraction directory");

    // Extract all objects using llvm-ar
    let extract_status = Command::new("llvm-ar")
        .arg("x")
        .arg(&static_lib)
        .current_dir(&extract_dir)
        .status()
        .expect("Failed to extract archive with llvm-ar");

    if !extract_status.success() {
        // Try with regular ar if llvm-ar failed
        let extract_status = Command::new("ar")
            .arg("x")
            .arg(&static_lib)
            .current_dir(&extract_dir)
            .status()
            .expect("Failed to extract archive with ar");
        
        if !extract_status.success() {
            panic!("Failed to extract archive {}", static_lib.display());
        }
    }

    // Get list of extracted object files
    let extracted_files: Vec<_> = fs::read_dir(&extract_dir)
        .expect("Failed to read extraction directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("obj") || ext == Some("o") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // For Windows, we'll apply a simpler approach - just rebuild the library
    // Full symbol filtering would require more complex tooling
    let mut llvm_lib_cmd = Command::new("llvm-lib");
    llvm_lib_cmd.arg("/OUT:").arg(&final_lib);
    
    for obj in extracted_files {
        llvm_lib_cmd.arg(&obj);
    }

    let lib_status = llvm_lib_cmd.status()
        .expect("Failed to execute llvm-lib command");

    if !lib_status.success() {
        panic!("llvm-lib command failed when creating {}", final_lib.display());
    }

    // Cleanup
    let _ = fs::remove_dir_all(&extract_dir);
}

fn patch_lib_windows_msvc(static_lib: &Path, out_dir: &Path, lib_name: &str, _lib_name_patched: &str, final_lib: &Path) {
    // Extract objects using Microsoft's lib.exe
    let extract_dir = out_dir.join(format!("{}_extracted", lib_name));
    fs::create_dir_all(&extract_dir).expect("Failed to create extraction directory");

    // List contents first to see what's available
    let list_output = Command::new("lib")
        .arg("/LIST")
        .arg(&static_lib)
        .output()
        .expect("Failed to list library contents");

    if !list_output.status.success() {
        panic!("Failed to list library contents for {}", static_lib.display());
    }

    // Extract each object file
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    let mut extracted_files = Vec::new();
    
    for line in list_stdout.lines() {
        let line = line.trim();
        if !line.is_empty() && (line.ends_with(".obj") || line.ends_with(".o")) {
            let obj_name = line;
            let extracted_path = extract_dir.join(obj_name);
            
            let extract_status = Command::new("lib")
                .arg("/EXTRACT:")
                .arg(obj_name)
                .arg("/OUT:")
                .arg(&extracted_path)
                .arg(&static_lib)
                .status()
                .expect("Failed to extract object from library");
            
            if extract_status.success() {
                extracted_files.push(extracted_path);
            }
        }
    }

    if extracted_files.is_empty() {
        // Fallback to copying the whole library
        println!("cargo:warning=Could not extract objects from Windows library, copying as-is");
        fs::copy(static_lib, final_lib)
            .expect("Failed to copy library");
        let _ = fs::remove_dir_all(&extract_dir);
        return;
    }

    // Recreate the library with the extracted objects
    let mut lib_cmd = Command::new("lib");
    lib_cmd.arg("/OUT:").arg(&final_lib);
    
    for obj in extracted_files {
        lib_cmd.arg(&obj);
    }

    let lib_status = lib_cmd.status()
        .expect("Failed to execute lib command");

    if !lib_status.success() {
        panic!("lib command failed when creating {}", final_lib.display());
    }

    // Cleanup
    let _ = fs::remove_dir_all(&extract_dir);
}


// Helper function to parse nm output on macOS and extract symbol names
fn parse_nm_symbol_macos(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    // macOS nm output format: [address] [type] [symbol]
    // Types we care about: T (text), D (data), B (bss), S (other section)
    if parts.len() >= 3 {
        let symbol_type = parts[1];
        let symbol_name = parts[2];
        
        // Check if this is a defined global symbol
        // Capital letters indicate global symbols
        if symbol_type.chars().any(|c| c.is_uppercase()) {
            Some(symbol_name.to_string())
        } else {
            None
        }
    } else {
        None
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
