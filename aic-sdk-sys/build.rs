use std::env;
use std::path::PathBuf;

fn main() {
    // Get the directory of the current build script.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Construct the path to the 'libs' directory relative to the manifest directory.
    let lib_dir = manifest_dir.join("libs");

    // Tell cargo where to find the native library.
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Tell cargo to link against the 'aic' library statically.
    // Cargo automatically handles the platform-specific library name (.a or .lib).
    println!("cargo:rustc-link-lib=static=aic");

    // Tell cargo to rerun the build script if the library changes.
    println!(
        "cargo:rerun-if-changed={}",
        lib_dir.join("libaic.a").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        lib_dir.join("aic.lib").display()
    ); // For Windows
}
