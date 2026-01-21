use crate::error::*;

use aic_sdk_sys::*;

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    path::Path,
    ptr,
};

/// High-level wrapper for the ai-coustics audio enhancement model.
///
/// This struct provides a safe, Rust-friendly interface to the underlying C library.
/// It handles memory management automatically and converts C-style error codes
/// to Rust `Result` types.
///
/// # Sharing and Multi-threading
///
/// `Model` is `Send` and `Sync`, so you can share it across threads. It does not implement
/// `Clone`, so wrap it in an `Arc` if you need shared ownership.
///
/// # Example
///
/// ```rust,no_run
/// # use aic_sdk::{Model, ProcessorConfig, Processor};
/// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
/// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
/// let mut processor = Processor::new(&model, &license_key).unwrap();
/// processor.initialize(&config).unwrap();
/// let mut audio_buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
/// processor.process_interleaved(&mut audio_buffer).unwrap();
/// ```
///
/// # Multi-threaded Example
///
/// ```rust,no_run
/// # use aic_sdk::{Model, ProcessorConfig, Processor};
/// # use std::{thread, sync::Arc};
/// let model = Arc::new(Model::from_file("/path/to/model.aicmodel").unwrap());
///
/// // Spawn multiple threads, each with its own processor but sharing the same model
/// let handles: Vec<_> = (0..4)
///     .map(|i| {
///         let model_clone = Arc::clone(&model);
///         thread::spawn(move || {
///             let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
///             let mut processor = Processor::new(&model_clone, &license_key).unwrap();
///             // Process audio in this thread...
///         })
///     })
///     .collect();
///
/// for handle in handles {
///     handle.join().unwrap();
/// }
/// ```
pub struct Model<'a> {
    /// Raw pointer to the C model structure
    ptr: *mut AicModel,
    /// Marker to tie the lifetime of the model to the lifetime of its weights
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Model<'a> {
    /// Creates a new audio enhancement model instance.
    ///
    /// Multiple models can be created to process different audio streams simultaneously
    /// or to switch between different enhancement algorithms during runtime.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to a model file.
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
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Model<'static>, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_path = CString::new(path.as_ref().to_string_lossy().as_bytes()).unwrap();

        // SAFETY:
        // - `model_ptr` points to stack memory we own.
        // - `c_path` is a valid, null-terminated string.
        let error_code = unsafe { aic_model_create_from_file(&mut model_ptr, c_path.as_ptr()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Model {
            ptr: model_ptr,
            marker: PhantomData,
        })
    }

    /// Creates a new model instance from an in-memory buffer.
    ///
    /// The buffer must be 64-byte aligned.
    ///
    /// Consider using [`include_model!`](macro@crate::include_model) to embed a model file at compile time with
    /// the correct alignment.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Raw bytes of the model file.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Model` instance or an `AicError` if creation fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use aic_sdk::{include_model, Model};
    /// static MODEL: &'static [u8] = include_model!("/path/to/model.aicmodel");
    /// let model = Model::from_buffer(MODEL).unwrap();
    /// ```
    pub fn from_buffer(buffer: &'a [u8]) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();

        // SAFETY:
        // - `buffer` is a valid slice and immutable for `'a`.
        // - The SDK only reads from `buffer` for the lifetime of the model.
        let error_code =
            unsafe { aic_model_create_from_buffer(&mut model_ptr, buffer.as_ptr(), buffer.len()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Model {
            ptr: model_ptr,
            marker: PhantomData,
        })
    }

    /// Returns the model identifier string.
    pub fn id(&self) -> &str {
        // SAFETY: `self` owns a valid model pointer created by the SDK.
        let id_ptr = unsafe { aic_model_get_id(self.as_const_ptr()) };
        if id_ptr.is_null() {
            return "unknown";
        }

        // SAFETY: Pointer is valid for the lifetime of `self` and is null-terminated.
        unsafe { CStr::from_ptr(id_ptr).to_str().unwrap_or("unknown") }
    }

    /// Retrieves the native sample rate of the processor's model.
    ///
    /// Each model is optimized for a specific sample rate, which determines the frequency
    /// range of the enhanced audio output. While you can process audio at any sample rate,
    /// understanding the model's native rate helps predict the enhancement quality.
    ///
    /// **How sample rate affects enhancement:**
    /// - Models trained at lower sample rates (e.g., 8 kHz) can only enhance frequencies
    ///   up to their Nyquist limit (4 kHz for 8 kHz models)
    /// - When processing higher sample rate input (e.g., 48 kHz) with a lower-rate model,
    ///   only the lower frequency components will be enhanced
    ///
    /// **Enhancement blending:**
    /// When enhancement strength is set below 1.0, the enhanced signal is blended with
    /// the original, maintaining the full frequency spectrum of your input while adding
    /// the model's noise reduction capabilities to the lower frequencies.
    ///
    /// **Sample rate and optimal frames relationship:**
    /// When using different sample rates than the model's native rate, the optimal number
    /// of frames (returned by `optimal_num_frames`) will change. The model's output
    /// delay remains constant regardless of sample rate as long as you use the optimal frame
    /// count for that rate.
    ///
    /// **Recommendation:**
    /// For maximum enhancement quality across the full frequency spectrum, match your
    /// input sample rate to the model's native rate when possible.
    ///
    /// # Returns
    ///
    /// Returns the model's native sample rate.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let optimal_sample_rate = model.optimal_sample_rate();
    /// println!("Optimal sample rate: {optimal_sample_rate} Hz");
    /// ```
    pub fn optimal_sample_rate(&self) -> u32 {
        let mut sample_rate: u32 = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live model.
        // - `sample_rate` points to stack storage for output.
        let error_code =
            unsafe { aic_model_get_optimal_sample_rate(self.as_const_ptr(), &mut sample_rate) };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_get_optimal_sample_rate` is documented to always succeed if given a valid processor pointer.
        assert_success(
            error_code,
            "`aic_model_get_optimal_sample_rate` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        // This should never fail
        sample_rate
    }

    /// Retrieves the optimal number of frames for the selected model at a given sample rate.
    ///
    ///
    /// Using the optimal number of frames minimizes latency by avoiding internal buffering.
    ///
    /// **When you use a different frame count than the optimal value, the model will
    /// introduce additional buffering latency on top of its base processing delay.**
    ///
    /// The optimal frame count varies based on the sample rate. Each model operates on a
    /// fixed time window duration, so the required number of frames changes with sample rate.
    /// For example, a model designed for 10 ms processing windows requires 480 frames at
    /// 48 kHz, but only 160 frames at 16 kHz to capture the same duration of audio.
    ///
    /// Call this function with your intended sample rate before calling
    /// [`Processor::initialize`](crate::Processor::initialize) to determine the best frame count for minimal latency.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The sample rate in Hz for which to calculate the optimal frame count.
    ///
    /// # Returns
    ///
    /// Returns the optimal frame count.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let sample_rate = model.optimal_sample_rate();
    /// let optimal_frames = model.optimal_num_frames(sample_rate);
    /// println!("Optimal frame count: {optimal_frames}");
    /// ```
    pub fn optimal_num_frames(&self, sample_rate: u32) -> usize {
        let mut num_frames: usize = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live model.
        // - `num_frames` points to stack storage for output.
        let error_code = unsafe {
            aic_model_get_optimal_num_frames(self.as_const_ptr(), sample_rate, &mut num_frames)
        };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_get_optimal_num_frames` is documented to always succeed if given valid pointers.
        assert_success(
            error_code,
            "`aic_model_get_optimal_num_frames` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        num_frames
    }

    /// Downloads a model file from the ai-coustics artifact CDN.
    ///
    /// This method fetches the model manifest, verifies that the requested model
    /// exists in a version compatible with this library, and downloads the model
    /// file to the specified directory. If the model file already exists, it will not
    /// be re-downloaded. If the existing file's checksum does not match, the model will
    /// be downloaded and the existing file will be replaced.
    /// 
    /// The manifest file is not cached and will always be downloaded on every call
    /// to ensure the latest model versions are always used.
    ///
    /// Available models can be browsed at [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io/).
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model identifier (e.g., `"quail-l-16khz"`).
    /// * `download_dir` - Directory where the model file will be stored.
    ///
    /// # Returns
    ///
    /// Returns the full path to the model file on success, or an [`AicError`] if the
    /// operation fails.
    ///
    /// # Note
    ///
    /// This is a blocking operation that performs network I/O.
    #[cfg(feature = "download-model")]
    pub fn download<P: AsRef<Path>>(
        model_id: &str,
        download_dir: P,
    ) -> Result<std::path::PathBuf, AicError> {
        let compatible_version = crate::get_compatible_model_version();
        crate::download::download(model_id, compatible_version, download_dir)
            .map_err(|err| AicError::ModelDownload(err.to_string()))
    }

    pub(crate) fn as_const_ptr(&self) -> *const AicModel {
        self.ptr as *const AicModel
    }
}

impl<'a> Drop for Model<'a> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY:
            // - `self.ptr` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_model_destroy(self.ptr) };
        }
    }
}

// SAFETY:
// - Model wraps a raw pointer to an AicModel which is immutable after creation and it
//   does not provide access to it through its public API.
// - Methods only pass the pointer to SDK calls documented as thread-safe for const access.
unsafe impl<'a> Send for Model<'a> {}
// SAFETY:
// - Model wraps a raw pointer to an AicModel which is immutable after creation and it
//   does not provide access to it through its public API.
// - Methods only pass the pointer to SDK calls documented as thread-safe for const access.
unsafe impl<'a> Sync for Model<'a> {}

/// Embeds the bytes of model file, ensuring proper alignment.
///
/// This macro uses Rust's standard library's [`include_bytes!`](std::include_bytes) macro
/// to include the model file at compile time.
///
/// # Example
///
/// ```rust,ignore
/// # use aic_sdk::{include_model, Model};
///
/// static MODEL: &'static [u8] = include_model!("/path/to/model.aicmodel");
/// let model = Model::from_buffer(MODEL).unwrap();
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
    use super::*;

    #[test]
    fn include_model_aligns_to_64_bytes() {
        // Use the README.md as a dummy file for testing
        let data = include_model!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"));

        let ptr = data.as_ptr() as usize;
        assert!(
            ptr.is_multiple_of(64),
            "include_model should align data to 64 bytes"
        );
    }

    #[test]
    fn model_is_send_and_sync() {
        // Compile-time check that Model implements Send and Sync.
        // This ensures the model can be safely shared across threads.
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Model>();
        assert_sync::<Model>();
    }
}

#[doc(hidden)]
mod _compile_fail_tests {
    //! Compile-fail regression: a `Model` created from a buffer must not outlive the buffer.
    //!
    //! ```rust,compile_fail
    //! use aic_sdk::Model;
    //!
    //! fn leak_model_from_buffer() -> Model<'static> {
    //!     let bytes = vec![0u8; 64];
    //!     let model = Model::from_buffer(&bytes).unwrap();
    //!     model
    //! }
    //!
    //! fn main() {
    //!     let _ = leak_model_from_buffer();
    //! }
    //! ```
}
