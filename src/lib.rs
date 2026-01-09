#![doc = include_str!("../README.md")]
use aic_sdk_sys::{aic_get_compatible_model_version, aic_get_sdk_version};
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
/// # Safety
/// The returned pointer points to a static string and remains valid
/// for the lifetime of the program. The caller should NOT free this pointer.
///
/// # Returns
///
/// Returns the library version as a string, or `None` if the version cannot be retrieved.
///
/// # Example
///
/// ```rust
/// let version = aic_sdk::get_sdk_version();
/// println!("ai-coustics SDK version: {version}");
/// ```
pub fn get_sdk_version() -> &'static str {
    // SAFETY: FFI call returns a pointer to a static C string owned by the SDK.
    // The pointer can never be null, so no check is necessary.
    let version_ptr = unsafe { aic_get_sdk_version() };

    // SAFETY: Pointer either came from the SDK or we already bailed if it was null.
    unsafe { CStr::from_ptr(version_ptr).to_str().unwrap_or("unknown") }
}

/// Returns the model version number compatible with this SDK build.
pub fn get_compatible_model_version() -> u32 {
    // SAFETY: FFI call takes no arguments and returns a plain integer.
    unsafe { aic_get_compatible_model_version() }
}
