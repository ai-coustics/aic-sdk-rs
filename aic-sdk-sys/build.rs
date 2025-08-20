use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(feature = "download-lib")]
#[path = "build-utils/downloader.rs"]
mod downloader;

fn main() {
    // Check if we're on Linux
    if !cfg!(target_os = "linux") {
        panic!("This build script only supports Linux for now");
    }

    if env::var("DOCS_RS").is_ok() {
        // On docs.rs we don't need to link, and B) we don't have network, so we couldn't download anything if we wanted to
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

    // link with the curated library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static={lib_name_patched}");

    generate_bindings();
}

#[cfg(feature = "download-lib")]
fn download_lib() -> PathBuf {
    use downloader::Downloader;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();

    let downloader = Downloader::new(&out_dir);
    downloader.download(&version)
}

fn patch_lib(lib_path: &Path, lib_name: &str, lib_name_patched: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let static_lib = lib_path.join(format!("lib{}.a", lib_name));

    let global_symbols_wildcard = "aic_*";

    if !static_lib.exists() {
        panic!("Please provide the SDK at {}", static_lib.display());
    }

    // original .o file
    let intermediate_obj = out_dir.join(format!("lib{}.o", lib_name));

    // modified .o file
    let final_obj = out_dir.join(format!("lib{}.o", lib_name_patched));

    // .a file
    let final_lib = out_dir.join(format!("lib{}.a", lib_name_patched));

    // partially link
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

    // curate symbols (only keep specific symbols)
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

    // build the archive
    let ar_status = Command::new("ar")
        .arg("rcs")
        .arg(&final_lib)
        .arg(&final_obj)
        .status()
        .expect("Failed to execute ar.");

    if !ar_status.success() {
        panic!("objcopy command failed for {}", final_obj.display());
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
