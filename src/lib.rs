#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

use aic_sdk_sys::{aic_get_compatible_model_version, aic_get_sdk_version, aic_set_sdk_wrapper_id};
use std::{ffi::CStr, sync::Once};

#[cfg(feature = "runtime-linking")]
use std::path::Path;

mod analyzer;
mod error;
mod file_analyzer;
mod model;
mod processor;
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
mod processor_async;
mod vad;

pub use analyzer::*;
pub use error::*;
pub use file_analyzer::*;
pub use model::*;
pub use processor::*;
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use processor_async::*;
pub use vad::*;

#[cfg(feature = "runtime-linking")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime-linking")))]
pub use aic_sdk_sys::DynamicLoadingError;

static SET_WRAPPER_ID: Once = Once::new();

/// Sets the SDK wrapper ID.
pub(crate) fn set_wrapper_id() {
    SET_WRAPPER_ID.call_once(|| unsafe {
        // SAFETY:
        // - This FFI call has no safety requirements.
        aic_set_sdk_wrapper_id(2);
    });
}

/// Loads the AIC dynamic library from `path` when the `runtime-linking` feature is enabled.
///
/// This is optional. With `runtime-linking`, the library is loaded automatically on first use
/// from the platform default name (`libaic.so` / `libaic.dylib` / `aic.dll`) via the OS loader
/// search path. Call this only to pick a specific file, and do so before the first SDK call.
///
/// # Safety
///
/// `path` must point to an AIC dynamic library that is ABI-compatible with this crate's bundled
/// `aic.h` header. Loading an incompatible library can cause undefined behavior when SDK functions
/// are called.
#[cfg(feature = "runtime-linking")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime-linking")))]
pub unsafe fn load_library<P: AsRef<Path>>(path: P) -> Result<(), DynamicLoadingError> {
    unsafe { aic_sdk_sys::load_library(path) }
}

/// Returns whether an AIC dynamic library has already been loaded.
#[cfg(feature = "runtime-linking")]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime-linking")))]
pub fn is_library_loaded() -> bool {
    aic_sdk_sys::is_library_loaded()
}

/// Returns the version of the ai-coustics SDK library.
///
/// # Note
/// This is not necessarily the same as this crate's version.
///
/// # Returns
///
/// Returns the SDK version string, or `"unknown"` if it cannot be decoded.
///
/// # Example
///
/// ```rust
/// let version = aic_sdk::get_sdk_version();
/// println!("ai-coustics SDK version: {version}");
/// ```
pub fn get_sdk_version() -> &'static str {
    // SAFETY:
    // - FFI call returns a pointer to a static C string owned by the SDK.
    // - The pointer can never be null, so no check is necessary.
    let version_ptr = unsafe { aic_get_sdk_version() };

    // SAFETY:
    // - SDK returns a null-terminated static string.
    unsafe { CStr::from_ptr(version_ptr).to_str().unwrap_or("unknown") }
}

/// Returns the model version number compatible with this SDK build.
pub fn get_compatible_model_version() -> u32 {
    // SAFETY:
    // - FFI call takes no arguments and returns a plain integer.
    unsafe { aic_get_compatible_model_version() }
}

/// This function is only used to identify SDKs by ai-coustics and should not be called by users of this crate.
///
/// # Safety
///
/// - Don't call this function unless you know what you're doing.
pub unsafe fn set_sdk_id(id: u32) {
    // SAFETY:
    // - This FFI call has no safety requirements.
    unsafe { aic_set_sdk_wrapper_id(id) }
}
