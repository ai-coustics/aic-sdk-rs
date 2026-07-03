//! Helpers for selecting the native library to link against. Kept in its own module so the pure
//! path logic can be unit tested (registered as a `[[test]]` target in `Cargo.toml`), the same way
//! `downloader.rs` is.

/// Selects the CRT-specific subdirectory of the SDK `lib` folder that holds the MSVC static
/// library. The SDK ships `aic.lib` twice, built for each C runtime flavour, and the linked copy
/// must match the CRT the Rust toolchain uses, otherwise CRT symbols clash at link time.
///
/// `target_features` is the comma-separated `CARGO_CFG_TARGET_FEATURE` value. The `crt-static`
/// feature (`RUSTFLAGS="-C target-feature=+crt-static"`, i.e. `/MT`) selects `static-crt`; its
/// absence is the MSVC default (`/MD`) and selects `dynamic-crt`.
pub fn msvc_static_crt_subdir(target_features: &str) -> &'static str {
    let crt_static = target_features.split(',').any(|feature| feature == "crt-static");
    if crt_static {
        "static-crt"
    } else {
        "dynamic-crt"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dynamic_crt() {
        // The MSVC default links the dynamic CRT (`/MD`); `crt-static` is absent.
        assert_eq!(msvc_static_crt_subdir(""), "dynamic-crt");
        assert_eq!(msvc_static_crt_subdir("fxsr,sse,sse2"), "dynamic-crt");
    }

    #[test]
    fn static_crt_when_feature_present() {
        assert_eq!(msvc_static_crt_subdir("crt-static"), "static-crt");
        assert_eq!(
            msvc_static_crt_subdir("fxsr,sse,sse2,crt-static"),
            "static-crt",
        );
    }

    #[test]
    fn does_not_match_substring() {
        // A feature that merely contains the text must not be treated as `crt-static`.
        assert_eq!(msvc_static_crt_subdir("crt-static-foo"), "dynamic-crt");
    }
}
