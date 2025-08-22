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
    if cfg!(target_os = "windows") {
        panic!("Windows is not supported yet. Please use a different platform to build the SDK.");
    }

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

    let static_lib_ext = if cfg!(target_os = "windows") {
        ".lib"
    } else {
        ".a"
    };
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
        patch_linux::patch_lib(
            &static_lib,
            &out_dir,
            lib_name,
            lib_name_patched,
            global_symbols_wildcard,
            &final_lib,
        );
    } else if cfg!(target_os = "macos") {
        patch_macos::patch_lib(
            &static_lib,
            &out_dir,
            lib_name,
            lib_name_patched,
            global_symbols_wildcard,
            &final_lib,
        );
    } else if cfg!(target_os = "windows") {
        patch_windows::patch_lib(
            &static_lib,
            &out_dir,
            lib_name,
            lib_name_patched,
            global_symbols_wildcard,
            &final_lib,
        );
    } else {
        panic!("Unsupported platform for library patching");
    }

    // Rerun this script if the static library changes.
    println!("cargo:rerun-if-changed={}", static_lib.display());
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
