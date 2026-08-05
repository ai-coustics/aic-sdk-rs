use crate::error::*;

use aic_sdk_sys::*;

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    path::Path,
    ptr,
};

/// High-level wrapper for an ai-coustics model.
///
/// A single model instance can be used to create multiple processors, VADs or analyzers,
/// according to the model type.
///
/// Each processor, VAD or analyzer created with a given model keeps the underlying model
/// alive through internal reference counting. When the reference count reaches zero the
/// model is destroyed. You may therefore drop the model before those objects, in any order.
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
/// let model = Model::from_file("/path/to/model.aicmodel")?;
/// let config = ProcessorConfig::optimal(&model);
/// let mut processor = Processor::new(&model, &license_key)?;
/// processor.initialize(&config)?;
/// let mut audio_block = vec![0.0f32; config.block_size];
/// processor.process(&mut audio_block)?;
/// # Ok::<(), aic_sdk::AicError>(())
/// ```
///
/// # Multi-threaded Example
///
/// ```rust,no_run
/// # use aic_sdk::{Model, ProcessorConfig, Processor};
/// # use std::{thread, sync::Arc};
/// let model = Arc::new(Model::from_file("/path/to/model.aicmodel")?);
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
/// # Ok::<(), aic_sdk::AicError>(())
/// ```
pub struct Model<'a> {
    /// Raw pointer to the C model structure
    ptr: *mut AicModel,
    /// Marker to tie the lifetime of the model to the lifetime of its weights
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Model<'a> {
    /// Creates a new model instance from a model file.
    ///
    /// A single model instance can be used to create multiple processors, VADs or analyzers,
    /// according to the model type.
    ///
    /// # Lifetime and ownership
    ///
    /// Each processor, VAD or analyzer created with a given model keeps the underlying model
    /// alive through internal reference counting. When the reference count reaches zero the
    /// model is destroyed. You may therefore drop the model before those objects, in any order.
    ///
    /// The model data is memory-mapped from the file, not copied into the process. Make sure
    /// the file is not modified or deleted while the model, or any object created from it, is
    /// alive.
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
    /// let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Model<'static>, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_path = CString::new(path.as_ref().to_string_lossy().as_bytes()).unwrap();

        // SAFETY:
        // - `model_ptr` points to stack memory we own.
        // - `c_path` is a valid, null-terminated string.
        // - This function is not thread-safe, but the output pointer and path
        //   buffer are local to this call and not shared with other threads.
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

    /// Creates a new model instance from a memory buffer.
    ///
    /// A single model instance can be used to create multiple processors, VADs or analyzers,
    /// according to the model type.
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
    /// let model = Model::from_buffer(MODEL)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn from_buffer(buffer: &'a [u8]) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();

        // SAFETY:
        // - `buffer` is a valid slice and immutable for `'a`.
        // - The SDK only reads from `buffer` for the lifetime of the model.
        // - This function is not thread-safe, but the output pointer is local to
        //   this call and no model handle exists until it returns.
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

    /// Returns the model identifier.
    ///
    /// The returned string is UTF-8 encoded.
    pub fn id(&self) -> &str {
        // SAFETY:
        // - `self` owns a valid model pointer created by the SDK.
        // - The returned pointer is only used while `self` keeps the model alive.
        // - This function is not thread-safe with concurrent destruction, which
        //   Rust prevents while `&self` is live.
        let id_ptr = unsafe { aic_model_get_id(self.as_const_ptr()) };
        if id_ptr.is_null() {
            return "unknown";
        }

        // SAFETY: Pointer is valid for the lifetime of `self` and is null-terminated.
        unsafe { CStr::from_ptr(id_ptr).to_str().unwrap_or("unknown") }
    }

    /// Retrieves the optimal sample rate of the model.
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
    /// **Sample rate and optimal block size relationship:**
    /// When using different sample rates than the model's native rate, the optimal samples
    /// per block (returned by [`Model::optimal_block_size`]) will change. The processor's output delay remains
    /// constant regardless of sample rate as long as you use the optimal block size for
    /// that rate.
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
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// let optimal_sample_rate = model.optimal_sample_rate();
    /// println!("Optimal sample rate: {optimal_sample_rate} Hz");
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn optimal_sample_rate(&self) -> u32 {
        let mut sample_rate: u32 = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live model.
        // - `sample_rate` points to stack storage for output.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code =
            unsafe { aic_model_get_optimal_sample_rate(self.as_const_ptr(), &mut sample_rate) };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_model_get_optimal_sample_rate` is documented to always succeed if given valid pointers.
        assert_success(
            error_code,
            "`aic_model_get_optimal_sample_rate` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        // This should never fail
        sample_rate
    }

    /// Retrieves the optimal block size for the model at a given sample rate.
    ///
    /// Using the optimal block size minimizes latency by avoiding internal buffering.
    ///
    /// **When you use a different block size than the optimal value, the processor will
    /// introduce additional buffering latency on top of its base processing delay.**
    ///
    /// The optimal block size varies based on the sample rate. Each model operates on a
    /// fixed time window length, so the required number of samples changes with sample rate.
    /// For example, a model designed for 10 ms processing windows requires 480 samples at
    /// 48 kHz, but only 160 samples at 16 kHz to capture the same duration of audio.
    ///
    /// Call this function with your intended sample rate before calling
    /// [`Processor::initialize`](crate::Processor::initialize) to determine the best block size for minimal latency.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The sample rate in Hz for which to calculate the optimal block size.
    ///
    /// # Returns
    ///
    /// Returns the optimal block size.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let sample_rate = model.optimal_sample_rate();
    /// let optimal_block_size = model.optimal_block_size(sample_rate);
    /// println!("Optimal block size: {optimal_block_size}");
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn optimal_block_size(&self, sample_rate: u32) -> usize {
        let mut block_size: usize = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live model.
        // - `block_size` points to stack storage for output.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code = unsafe {
            aic_model_get_optimal_block_size(self.as_const_ptr(), sample_rate, &mut block_size)
        };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_model_get_optimal_block_size` is documented to always succeed if given valid pointers.
        assert_success(
            error_code,
            "`aic_model_get_optimal_block_size` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        block_size
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
        aic_model_downloader::download(model_id, compatible_version, download_dir)
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
            // - This function is not thread-safe with concurrent model use, but
            //   `drop` has exclusive access to `self`.
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
/// let model = Model::from_buffer(MODEL)?;
/// # Ok::<(), aic_sdk::AicError>(())
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
