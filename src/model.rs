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
/// ```rust,no_run
/// # use aic_sdk::{Config, Model, Processor};
/// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
/// let mut processor = Processor::new(&model, &license_key).unwrap();
/// let config = Config {
///     num_channels: 2,
///     ..processor.optimal_config()
/// };
/// processor.initialize(&config).unwrap();
/// let mut audio_buffer = vec![0.0f32; config.num_channels * config.num_frames];
/// processor.process_interleaved(&mut audio_buffer).unwrap();
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
    /// ```rust,no_run
    /// # use aic_sdk::Model;
    /// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_path = CString::new(path.as_ref().to_string_lossy().as_bytes()).unwrap();

        // SAFETY:
        // - `model_ptr` points to stack memory we own
        // - `c_path` is a valid, NUL-terminated string.
        let error_code = unsafe { aic_model_create_from_file(&mut model_ptr, c_path.as_ptr()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self { inner: model_ptr })
    }

    pub fn from_buffer(buffer: &[u8]) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();

        // SAFETY:
        // - `buffer` is a valid slice; its pointer/len are passed verbatim to C which only reads.
        let error_code = unsafe {
            aic_model_create_from_buffer(&mut model_ptr, buffer.as_ptr(), buffer.len())
        };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self { inner: model_ptr })
    }

    /// Downloads a model file from the ai-coustics artifact CDN.
    ///
    /// This method fetches the model manifest, checks whether the requested model
    /// exists in a version compatible with this library, and downloads the model
    /// file into the provided directory.
    ///
    /// # Note
    ///
    /// This is a blocking operation.
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier as listed in the manifest (e.g. `"quail-l-16khz"`).
    /// * `download_dir` - Directory where the downloaded model file should be stored.
    ///
    /// # Returns
    ///
    /// Returns the full path to the downloaded model file, or an `AicError` if the
    /// operation fails.
    #[cfg(feature = "download-model")]
    pub fn download<P: AsRef<Path>>(
        model_id: &str,
        download_dir: P,
    ) -> Result<std::path::PathBuf, AicError> {
        let compatible_version = crate::get_compatible_model_version();
        aic_model_downloader::download(model_id, compatible_version, download_dir)
            .map_err(|err| AicError::ModelDownload(err.to_string()))
    }

    pub(crate) fn as_const_ptr(&self) -> *const AicModel {
        self.inner as *const AicModel
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_model_destroy(self.inner) };
        }
    }
}

// SAFETY: The Model struct can be safely sent and shared between threads
unsafe impl Send for Model {}
unsafe impl Sync for Model {}

/// Embeds the bytes of model file, ensuring proper alignment.
/// 
/// This macro uses Rust's standard library's [`include_bytes!`](std::include_bytes) macro
/// to include the model file at compile time.
///
/// # Example
///
/// ```
/// use aic_sdk::include_model;
///
/// static MODEL: &'static [u8] = include_model!("path/to/model.aicmodel");
/// ```
#[macro_export]
macro_rules! include_model {
    ($path:expr) => {{
        #[repr(C, align(64))]
        struct __Aligned<T: ?Sized>(T);

        const __DATA: &'static __Aligned<[u8; include_bytes!($path).len()]> =
            &__Aligned(*include_bytes!($path));

        &__DATA.0
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn include_model_aligns_to_64_bytes() {
        // Use the README.md as a dummy file for testing
        let data = include_model!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/README.md"
        ));

        let ptr = data.as_ptr() as usize;
        assert!(ptr.is_multiple_of(64), "include_model should align data to 64 bytes");
    }
}
