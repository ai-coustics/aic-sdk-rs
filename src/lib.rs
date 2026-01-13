#![doc = include_str!("../README.md")]
use aic_sdk_sys::{aic_get_compatible_model_version, aic_get_sdk_version, aic_set_sdk_wrapper_id};
use std::ffi::CStr;

#[cfg(feature = "download-model")]
mod download;
mod error;
mod model;
mod processor;
mod vad;

pub use error::*;
pub use model::*;
pub use processor::*;
pub use vad::*;

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
    // - SDK returns a NUL-terminated static string.
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
/// SAFETY:
/// - Don't call this function unless you know what you're doing.
pub unsafe fn set_sdk_id(id: u32) {
    // SAFETY:
    // - This FFI call has no safety requirements.
    unsafe { aic_set_sdk_wrapper_id(id) }
}
