#![doc = include_str!("../README.md")]
use aic_sdk_sys::{aic_get_compatible_model_version, aic_get_sdk_version};
use std::ffi::CStr;

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

/// Returns the model version number compatible with this SDK build.
pub fn get_compatible_model_version() -> u32 {
    unsafe { aic_get_compatible_model_version() }
}

#[doc(hidden)]
mod _compile_fail_tests {
    //! Compile-fail regression: a `Processor` must not outlive its `Model`.
    //! This currently compiles (showing a lifetime hole), but once fixed the
    //! snippet should fail to compile.
    //!
    //! ```rust,compile_fail
    //! use aic_sdk::{Model, Processor};
    //!
    //! fn leak_processor() -> Processor {
    //!     let license_key = "dummy-license";
    //!     let processor = {
    //!         let model = Model::from_file("some/path.aicmodel").unwrap();
    //!         Processor::new(&model, license_key).unwrap()
    //!     };
    //!     processor
    //! }
    //!
    //! fn main() {
    //!     let _ = leak_processor();
    //! }
    //! ```
}
