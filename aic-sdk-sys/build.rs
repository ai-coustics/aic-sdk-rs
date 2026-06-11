use std::env;
use std::path::PathBuf;

#[cfg(feature = "download-lib")]
#[path = "build-utils/downloader.rs"]
mod downloader;

#[path = "build-utils/runtime_linking.rs"]
mod runtime_linking;

fn main() {
    // Rerun the build script if the header file changes
    println!("cargo:rerun-if-changed=include/aic.h");

    // Rerun the build script if the AIC_LIB_PATH environment variable changes
    println!("cargo:rerun-if-env-changed=AIC_LIB_PATH");

    // Bindings need to be generated before early return on docs.rs
    generate_bindings();

    if env::var("DOCS_RS").is_ok() {
        // On docs.rs we don't need to link and we don't have network,
        // so we couldn't download anything if we wanted to
        return;
    }

    let runtime_linking = env::var("CARGO_FEATURE_RUNTIME_LINKING").is_ok();
    let dynamic_linking = env::var("CARGO_FEATURE_DYNAMIC_LINKING").is_ok();

    // `dynamic-linking` and `runtime-linking` select alternative linking strategies. Cargo
    // features are additive, so enabling both (e.g. via `--all-features`) is possible; in that
    // case runtime linking wins. Warn so the choice is not silently surprising.
    if runtime_linking && dynamic_linking {
        println!(
            "cargo:warning=Both `dynamic-linking` and `runtime-linking` are enabled; using \
             runtime linking. These features select alternative linking strategies and are not \
             meant to be combined."
        );
    }

    if runtime_linking {
        // Runtime linking resolves symbols from a user-provided dynamic library path,
        // so there is intentionally no build-time link step. If `dynamic-linking`
        // is also enabled through additive Cargo features, runtime linking wins.
        return;
    }

    let lib_path = if let Ok(path) = env::var("AIC_LIB_PATH") {
        PathBuf::from(path)
    } else {
        #[cfg(feature = "download-lib")]
        {
            let downloaded_path = download_lib();
            downloaded_path.join("lib")
        }
        #[cfg(not(feature = "download-lib"))]
        {
            panic!(
                "Enable feature `download-lib` or use a local library by setting the environment variable `AIC_LIB_PATH`"
            );
        }
    };

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // Link with the curated library
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    if dynamic_linking && target_env == "msvc" {
        // The MSVC SDK package ships the DLL import library as `aic.dll.lib` next to the static
        // `aic.lib`. A plain `dylib=aic` resolves to `aic.lib` (the static archive), which would
        // be linked statically and miss its system dependencies. Name the import library
        // verbatim so the linker binds against `aic.dll` instead.
        println!("cargo:rustc-link-lib=dylib:+verbatim=aic.dll.lib");
    } else {
        let link_kind = if dynamic_linking { "dylib" } else { "static" };
        println!("cargo:rustc-link-lib={link_kind}=aic");
    }

    // The platform system libraries below are transitive dependencies of the *static* AIC
    // library and must be linked into the final binary. A shared `libaic` already records its
    // own dependencies, so when linking dynamically we leave them out.
    if !dynamic_linking {
        add_platform_specific_libs();
    }
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
        println!("cargo:rustc-link-lib=oleaut32");
        println!("cargo:rustc-link-lib=crypt32");
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

fn generate_bindings() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let header_path = manifest_dir.join("include").join("aic.h");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Generate bindings using bindgen
    let mut builder = bindgen::Builder::default()
        // The input header we would like to generate bindings for.
        .header(header_path.to_str().unwrap())
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Generate constified enums to avoid name repetition
        .constified_enum_module("AicErrorCode")
        .constified_enum_module("AicProcessorParameter")
        .constified_enum_module("AicVadParameter");

    if env::var("CARGO_FEATURE_RUNTIME_LINKING").is_ok() {
        let runtime_bindings = builder
            .clone()
            .generate()
            .expect("Unable to generate runtime-linking symbols");
        runtime_linking::generate(&runtime_bindings, &out_path.join("runtime_symbols.rs"));

        // The runtime-linking module provides Rust functions with these names
        // that dispatch through libloading. Keep types/constants from bindgen,
        // but omit build-linked extern function declarations to avoid conflicts.
        builder = builder.blocklist_function("aic_.*");
    }

    let bindings = builder
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Tell cargo to rerun the build script if the library or header changes.
    println!("cargo:rerun-if-changed={}", header_path.display());
}
