use crate::error::*;

use aic_sdk_sys::*;

use std::{ffi::CString, path::Path, ptr};

/// High-level wrapper for the ai-coustics audio enhancement model.
///
/// This struct provides a safe, Rust-friendly interface to the underlying C library.
/// It handles memory management automatically and converts C-style error codes
/// to Rust `Result` types.
///
/// # Example
///
/// ```rust
/// use aic_sdk::{Model, ModelType};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
///
/// model.initialize(48000, 1, 1024, false).unwrap();
///
/// // Process audio data
/// let mut audio_buffer = vec![0.0f32; 1024];
/// model.process_interleaved(&mut audio_buffer).unwrap();
/// ```
pub struct Model {
    /// Raw pointer to the C model structure
    inner: *mut AicModel,
}

impl Model {
    /// Creates a new audio enhancement model instance.
    ///
    /// Multiple models can be created to process different audio streams simultaneously
    /// or to switch between different enhancement algorithms during runtime.
    ///
    /// # Arguments
    ///
    /// * `model_type` - Selects the enhancement algorithm variant
    /// * `license_key` - Valid license key for the AIC SDK
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Model` instance or an `AicError` if creation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_path = CString::new(path.as_ref().to_string_lossy().as_bytes()).unwrap();
        
        let error_code =
            unsafe { aic_model_create_from_file(&mut model_ptr, c_path.as_ptr()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: model_ptr,
        })
    }

    pub fn from_buffer(buffer: &[u8]) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        
        let error_code =
            unsafe { aic_model_create_from_buffer(&mut model_ptr, buffer.as_ptr(), buffer.len()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: model_ptr,
        })
    }

    pub(crate) fn as_const_ptr(&self) -> *const AicModel {
        self.inner as *const AicModel
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe {
                aic_model_destroy(self.inner);
            }
        }
    }
}

// SAFETY: The Model struct can be safely sent and shared between threads
unsafe impl Send for Model {}
unsafe impl Sync for Model {}
