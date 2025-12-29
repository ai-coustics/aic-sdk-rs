#![doc = include_str!("../README.md")]
use aic_sdk_sys::aic_get_sdk_version;
use std::ffi::CStr;

#[cfg(feature = "download-model")]
mod download;
mod error;
mod model;
mod processor;
mod vad;

#[cfg(feature = "download-model")]
pub use download::download_quail_xxs_48khz;
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
/// let version = aic_sdk::get_version();
/// println!("ai-coustics SDK version: {version}");
/// ```
pub fn get_version() -> &'static str {
    let version_ptr = unsafe { aic_get_sdk_version() };
    if version_ptr.is_null() {
        return "unknown";
    }

    unsafe { CStr::from_ptr(version_ptr).to_str().unwrap_or("unknown") }
}
