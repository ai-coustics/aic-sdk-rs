use std::env;
use std::path::PathBuf;
use std::process::Command;

#[cfg(not(target_os = "linux"))]
fn main() {
    panic!("This platform is currently not supported.")
}

#[cfg(target_os = "linux")]
fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let lib_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("libs");

    let lib_name = "aic";
    let global_symbols_wildcard = "aic_*";

    let static_lib = lib_path.join(format!("lib{}.a", lib_name));

    if !static_lib.exists() {
        panic!("Please provide the SDK at {}", static_lib.display());
    }

    // original .o file
    let intermediate_obj = out_dir.join(format!("lib{}.o", lib_name));

    // modified .o file
    let final_obj = out_dir.join(format!("lib{}_curated.o", lib_name));

    // .a file
    let final_lib = out_dir.join(format!("lib{}_curated.a", lib_name));

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

    // link with the curated library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!(
        "cargo:rustc-link-lib=static={}",
        format!("{}_curated", lib_name)
    );

    // Rerun this script if the static library changes.
    println!("cargo:rerun-if-changed={}", static_lib.display());
}
