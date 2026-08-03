use crate::{error::*, model::Model, processor::ProcessorConfig};

use aic_sdk_sys::*;

use std::{ffi::CString, marker::PhantomData, ptr};

/// The result of analyzing an audio signal with an [`Analyzer`].
///
/// Scores are in the range `0.0..=1.0`. For all fields except
/// [`speaker_loudness`](Self::speaker_loudness), lower values indicate less problematic audio.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    /// Headline audio score.
    ///
    /// Predicts likelihood of failure of downstream models including speech-to-text,
    /// voice activity detection or turn-taking or speech-to-speech models.
    /// Lower indicates less problematic audio.
    ///
    /// **Range:** 0.0 to 1.0
    pub risk_score: f32,
    /// Measure of speaker distance and reverberance.
    /// Lower indicates less problematic audio.
    ///
    /// **Range:** 0.0 to 1.0
    pub speaker_reverb: f32,
    /// Measure of speaker loudness.
    ///
    /// **Range:** 0.0 to 1.0
    pub speaker_loudness: f32,
    /// Measure of interference from additional speakers present in audio.
    /// Lower indicates less problematic audio.
    ///
    /// **Range:** 0.0 to 1.0
    pub interfering_speech: f32,
    /// Measure of interfering speech content from media devices,
    /// e.g. from TVs, radios, phones or else.
    /// Lower indicates less problematic audio.
    ///
    /// **Range:** 0.0 to 1.0
    pub media_speech: f32,
    /// Measure of ambient or environmental noise.
    /// Lower indicates less problematic audio.
    ///
    /// **Range:** 0.0 to 1.0
    pub noise: f32,
    /// Measure of audio dropouts or discontinuities in the stream,
    /// e.g. from packet loss, frame erasure, jitter or CPU overload.
    /// Lower indicates less problematic audio.
    ///
    /// **Range:** 0.0 to 1.0
    pub packet_loss: f32,
}

impl From<AicAnalysisResult> for AnalysisResult {
    fn from(value: AicAnalysisResult) -> Self {
        Self {
            risk_score: value.risk_score,
            speaker_reverb: value.speaker_reverb,
            speaker_loudness: value.speaker_loudness,
            interfering_speech: value.interfering_speech,
            media_speech: value.media_speech,
            noise: value.noise,
            packet_loss: value.packet_loss,
        }
    }
}

/// Creates a collector/analyzer pair for non-real-time analysis.
///
/// The collector is designed to be placed in the audio thread, buffering audio chunks for
/// later analysis.
///
/// The analyzer is designed to be run separately. Analysis models are computationally expensive
/// and cannot run in the audio thread. The analyzer has access to the audio buffered by the
/// collector, and it can access it safely across threads.
///
/// The collector retains a span of audio determined by the analysis model. As more samples
/// get collected, old audio is discarded.
///
/// # Arguments
///
/// * `model` - The loaded model instance. Must be an analysis model, otherwise
///   [`AicError::ModelTypeUnsupported`] is returned.
/// * `license_key` - license key for the ai-coustics SDK
///   (generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com/))
///
/// # Warning
///
/// This function allocates memory. Do not call it from audio processing threads.
///
/// # Example
///
/// ```rust,no_run
/// # use aic_sdk::Model;
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/model.aicmodel")?;
/// let (mut collector, mut analyzer) = aic_sdk::analyzer_pair(&model, &license_key)?;
/// # Ok::<(), aic_sdk::AicError>(())
/// ```
pub fn analyzer_pair<'a>(
    model: &Model<'a>,
    license_key: &str,
) -> Result<(Collector, Analyzer<'a>), AicError> {
    // Set the wrapper ID as soon as the user attempts to instantiate an analyzer
    crate::set_wrapper_id();

    let mut collector_ptr: *mut AicCollector = ptr::null_mut();
    let mut analyzer_ptr: *mut AicAnalyzer = ptr::null_mut();
    let c_license_key = CString::new(license_key).map_err(|_| AicError::LicenseFormatInvalid)?;

    // SAFETY:
    // - `collector_ptr` and `analyzer_ptr` point to stack storage for output.
    // - `model` is a valid SDK model pointer for the duration of the call.
    // - `c_license_key` is a null-terminated CString.
    // - This function is not thread-safe, but the output pointers are local to
    //   this call and neither handle exists until it returns.
    let error_code = unsafe {
        aic_analyzer_pair_create(
            &mut collector_ptr,
            &mut analyzer_ptr,
            model.as_const_ptr(),
            c_license_key.as_ptr(),
        )
    };

    handle_error(error_code)?;

    assert!(
        !collector_ptr.is_null(),
        "C library returned success but null collector pointer"
    );
    assert!(
        !analyzer_ptr.is_null(),
        "C library returned success but null analyzer pointer"
    );

    let collector = Collector::new(collector_ptr);
    let analyzer = Analyzer::new(analyzer_ptr, model);

    Ok((collector, analyzer))
}

/// Buffers audio for later analysis.
///
/// The collector is designed to be placed in the audio thread,
/// buffering audio chunks for the [`Analyzer`] to analyze later.
pub struct Collector {
    /// Raw pointer to the C collector structure.
    inner: *mut AicCollector,
    /// Whether `initialize` has been called.
    initialized: bool,
}

impl Collector {
    fn new(collector_ptr: *mut AicCollector) -> Self {
        Self {
            inner: collector_ptr,
            initialized: false,
        }
    }

    /// Configures the collector for specific audio settings.
    ///
    /// This function must be called before buffering any audio.
    /// Using the sample rate and block size returned by [`Model::optimal_sample_rate`] and
    /// [`Model::optimal_block_size`] avoids internal resampling and rebuffering.
    ///
    /// # Arguments
    ///
    /// * `config` - Audio buffering configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if initialization fails.
    ///
    /// # Warning
    /// Do not call from audio processing threads as this allocates memory.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (mut collector, _) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model);
    /// collector.initialize(&config)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn initialize(&mut self, config: &ProcessorConfig) -> Result<(), AicError> {
        // SAFETY:
        // - `self.inner` is a valid pointer to a live collector.
        // - This function is not thread-safe, so we borrow `&mut self`.
        let error_code = unsafe {
            aic_collector_initialize(
                self.inner,
                config.sample_rate,
                config.block_size,
                config.variable_block_size,
            )
        };

        handle_error(error_code)?;
        self.initialized = true;
        Ok(())
    }

    /// Buffers mono audio.
    ///
    /// # Arguments
    ///
    /// * `audio` - Mono audio block to be buffered. Must be exactly of size
    ///   `block_size`, or if `variable_block_size` was enabled, less than
    ///   the initialization value.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if buffering fails.
    ///
    /// # Real-time safety
    ///
    /// Real-time safe. Can be called from audio processing threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (mut collector, _) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model);
    /// collector.initialize(&config)?;
    /// let audio = vec![0.0f32; config.block_size];
    /// collector.buffer(&audio)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn buffer(&mut self, audio: &[f32]) -> Result<(), AicError> {
        if !self.initialized {
            return Err(AicError::ProcessorNotInitialized);
        }

        let audio_len = audio.len();

        // SAFETY:
        // - `self.inner` is a valid pointer to a live collector.
        // - `audio` points to a contiguous, readable f32 slice of length `audio_len`.
        // - This function is not thread-safe, so we borrow `&mut self`.
        let error_code = unsafe { aic_collector_buffer(self.inner, audio.as_ptr(), audio_len) };

        handle_error(error_code)
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            // - This function is not thread-safe with concurrent collector use,
            //   but `drop` has exclusive access to `self`.
            unsafe { aic_collector_destroy(self.inner) };
        }
    }
}

// SAFETY: Everything in Collector is Send, with the exception of the inner raw pointer.
// The Collector only uses the raw pointer according to the safety contracts of the
// unsafe APIs that require the pointer, and the Collector does not expose access to the
// raw pointer in any of its methods. Therefore, it safe to implement Send for Collector.
unsafe impl Send for Collector {}

// SAFETY: Collector does not expose any interior mutability, and all unsafe APIs that make use of
// the inner raw pointer uphold the thread safety contracts required by the unsafe APIs.
// Therefore, it is safe to implement Sync for Collector.
unsafe impl Sync for Collector {}

/// Runs an analysis model over the audio buffered by a [`Collector`].
///
/// The analyzer is designed to be run in a non-audio thread. Analysis models are computationally expensive
/// and cannot run in the audio thread. The analyzer has access to the audio buffered by the
/// collector, and it can access it safely across threads.
pub struct Analyzer<'a> {
    /// Raw pointer to the C analyzer structure.
    inner: *mut AicAnalyzer,
    /// Marker to tie the analyzer to the lifetime of the model's weights.
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Analyzer<'a> {
    fn new(analyzer_ptr: *mut AicAnalyzer, _model: &Model<'a>) -> Self {
        Self {
            inner: analyzer_ptr,
            marker: PhantomData,
        }
    }

    fn as_const_ptr(&self) -> *const AicAnalyzer {
        self.inner as *const AicAnalyzer
    }

    /// Clears all internal state and buffers.
    ///
    /// Call this when the audio stream is interrupted or when seeking
    /// to prevent mispredictions from previous audio content.
    ///
    /// This operates on both the analyzer and its collector.
    ///
    /// The [`Collector`] stays initialized to the configured settings.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if the reset fails.
    ///
    /// # Real-time safety
    ///
    /// Real-time safe. Can be called from audio processing threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::Model;
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (_, mut analyzer) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// analyzer.reset()?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn reset(&self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live analyzer.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code = unsafe { aic_analyzer_reset(self.as_const_ptr()) };
        handle_error(error_code)
    }

    /// Analyze the buffered signal.
    ///
    /// The analyzer runs a forward-pass of the analysis model with a fixed length of audio,
    /// determined by the model.
    ///
    /// If this function is called before the collector has buffered that length of audio,
    /// the analyzer will run the analysis with silence (zeros) in the tail of the input.
    ///
    /// # Returns
    ///
    /// Returns an [`AnalysisResult`] if successful, otherwise an [`AicError`].
    ///
    /// # Real-time safety
    ///
    /// This function is not real-time safe. Avoid calling it from audio threads.
    pub fn analyze_buffered(&mut self) -> Result<AnalysisResult, AicError> {
        let mut result = AicAnalysisResult {
            risk_score: 0.0,
            speaker_reverb: 0.0,
            speaker_loudness: 0.0,
            interfering_speech: 0.0,
            media_speech: 0.0,
            noise: 0.0,
            packet_loss: 0.0,
        };

        // SAFETY:
        // - `self.inner` is a valid pointer to a live analyzer.
        // - `result` points to stack storage for output.
        // - This function is not thread-safe, so we borrow `&mut self`.
        let error_code = unsafe { aic_analyzer_analyze_buffered(self.inner, &mut result) };
        handle_error(error_code)?;

        Ok(result.into())
    }

    /// Terminates the telemetry session associated with this analyzer.
    ///
    /// Once the request has been handled, the analyzer is no longer allowed to analyze
    /// buffered audio.
    ///
    /// This is meant for lifecycle management events. A telemetry session is stopped
    /// automatically when the [`Analyzer`] is dropped, so calling this is only necessary when
    /// the session must end before the analyzer itself goes out of scope.
    ///
    /// This blocks until the telemetry session is terminated, unless another session is still
    /// alive. In that case it returns early and termination happens asynchronously.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if termination cannot be requested.
    ///
    /// # Real-time safety
    ///
    /// This function is not real-time safe. It may block until the session is terminated.
    /// Avoid calling it from audio threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::Model;
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (_, mut analyzer) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// analyzer.terminate_session()?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn terminate_session(&mut self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.inner` is a valid pointer to a live analyzer.
        // - This function must not run concurrently with any other call taking the same
        //   analyzer handle, so we borrow `&mut self`.
        let error_code = unsafe { aic_analyzer_terminate_session(self.inner) };
        handle_error(error_code)
    }

    /// Replaces the bearer token on the analyzer.
    ///
    /// Use this when your license key is a JWT and needs to be refreshed before it expires.
    /// The analyzer handle stays valid, buffered spectra stay available, and the new token is
    /// used for all subsequent authentication against the ai-coustics backend.
    ///
    /// In-place updates are only supported when both the originally configured key and the
    /// new token are JWTs. If either side is not, the call returns
    /// [`AicError::TokenUpdateUnsupported`] and the existing token stays in use.
    ///
    /// On any error the call is a no-op: the previously active token stays in use and the
    /// telemetry session is unaffected. On success the swap is applied immediately and is **not**
    /// gated on backend acceptance. The token is only validated locally for format; if the
    /// backend later rejects it, the SDK retries it under backoff rather than rolling back, and
    /// analysis calls may be rejected if no accepted token arrives in time. Supplying a
    /// known-good token during that window recovers the session.
    ///
    /// # Arguments
    ///
    /// * `token` - The new JWT to install.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if the update fails.
    ///
    /// # Real-time safety
    ///
    /// This function is not real-time safe. It locks a mutex and allocates memory.
    /// Avoid calling it from audio threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::Model;
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (_, analyzer) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// let renewed_jwt = String::from("<JWT_BEARER_TOKEN>");
    /// analyzer.update_bearer_token(&renewed_jwt)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn update_bearer_token(&self, token: &str) -> Result<(), AicError> {
        let c_token = CString::new(token).map_err(|_| AicError::LicenseFormatInvalid)?;

        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live analyzer.
        // - `c_token` is a null-terminated CString that outlives the call.
        // - This function can run concurrently with collector buffering; Rust
        //   prevents concurrent analyze or destroy on the same analyzer handle.
        let error_code =
            unsafe { aic_analyzer_update_bearer_token(self.as_const_ptr(), c_token.as_ptr()) };
        handle_error(error_code)
    }
}

impl<'a> Drop for Analyzer<'a> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            // - This function is not thread-safe with concurrent analyzer use,
            //   but `drop` has exclusive access to `self`.
            unsafe { aic_analyzer_destroy(self.inner) };
        }
    }
}

// SAFETY: Everything in Analyzer is Send, with the exception of the inner raw pointer.
// The Analyzer only uses the raw pointer according to the safety contracts of the
// unsafe APIs that require the pointer, and the Analyzer does not expose access to the
// raw pointer in any of its methods. Therefore, it safe to implement Send for Analyzer.
unsafe impl<'a> Send for Analyzer<'a> {}

// SAFETY: Analyzer does not expose any interior mutability, and all unsafe APIs that make use of
// the inner raw pointer uphold the thread safety contracts required by the unsafe APIs.
// Therefore, it is safe to implement Sync for Analyzer.
unsafe impl<'a> Sync for Analyzer<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    fn download_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn find_existing_model(target_dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(target_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.contains("tyto_l_16khz") && name.ends_with(".aicmodel"))
                .unwrap_or(false)
                && path.is_file()
            {
                return Some(path);
            }
        }
        None
    }

    /// Downloads the default test model `tyto-l-16khz` into the crate's `target/` directory.
    /// Returns the path to the downloaded model file.
    fn get_tyto_l_16khz() -> Result<PathBuf, AicError> {
        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");

        if let Some(existing) = find_existing_model(&target_dir) {
            return Ok(existing);
        }

        let _guard = download_lock().lock().unwrap();
        if let Some(existing) = find_existing_model(&target_dir) {
            return Ok(existing);
        }

        if cfg!(feature = "download-model") {
            Model::download("tyto-l-16khz", target_dir)
        } else {
            panic!(
                "Model `tyto-l-16khz` not found in {} and `download-model` feature is disabled",
                target_dir.display()
            );
        }
    }

    fn load_test_model() -> Result<(Model<'static>, String), AicError> {
        let license_key = std::env::var("AIC_SDK_LICENSE")
            .expect("AIC_SDK_LICENSE environment variable must be set for tests");

        let model_path = get_tyto_l_16khz()?;
        let model = Model::from_file(&model_path)?;

        Ok((model, license_key))
    }

    fn test_analyzer_pair(
        model: &Model<'static>,
        license_key: &str,
    ) -> (Collector, Analyzer<'static>) {
        analyzer_pair(model, license_key)
            .expect("tyto-l-16khz should create a collector/analyzer pair")
    }

    fn assert_score_range(result: &AnalysisResult) {
        assert!((0.0..=1.0).contains(&result.risk_score));
        assert!((0.0..=1.0).contains(&result.speaker_reverb));
        assert!((0.0..=1.0).contains(&result.speaker_loudness));
        assert!((0.0..=1.0).contains(&result.interfering_speech));
        assert!((0.0..=1.0).contains(&result.media_speech));
        assert!((0.0..=1.0).contains(&result.noise));
        assert!((0.0..=1.0).contains(&result.packet_loss));
    }

    #[test]
    fn analysis_result_maps_all_ffi_fields() {
        let ffi_result = AicAnalysisResult {
            risk_score: 0.1,
            speaker_reverb: 0.2,
            speaker_loudness: 0.3,
            interfering_speech: 0.4,
            media_speech: 0.5,
            noise: 0.6,
            packet_loss: 0.7,
        };

        assert_eq!(
            AnalysisResult::from(ffi_result),
            AnalysisResult {
                risk_score: 0.1,
                speaker_reverb: 0.2,
                speaker_loudness: 0.3,
                interfering_speech: 0.4,
                media_speech: 0.5,
                noise: 0.6,
                packet_loss: 0.7,
            }
        );
    }

    #[test]
    fn collector_rejects_buffering_before_initialize() {
        let mut collector = Collector {
            inner: ptr::null_mut(),
            initialized: false,
        };

        let audio = vec![0.0f32; 4];

        assert_eq!(
            collector.buffer(&audio),
            Err(AicError::ProcessorNotInitialized)
        );
    }

    #[test]
    fn analyzer_pair_rejects_license_key_with_nul() {
        let (model, _) = load_test_model().unwrap();

        let result = analyzer_pair(&model, "invalid\0license");

        assert!(matches!(result, Err(AicError::LicenseFormatInvalid)));
    }

    #[test]
    fn collector_buffers_audio_and_analyzer_returns_scores() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, mut analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model);
        collector.initialize(&config).unwrap();

        let audio = vec![0.0f32; config.block_size];
        collector.buffer(&audio).unwrap();

        let result = analyzer.analyze_buffered().unwrap();
        assert_score_range(&result);
    }

    #[test]
    fn collector_buffers_variable_block_size_when_enabled() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, _analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model).with_variable_block_size(true);
        collector.initialize(&config).unwrap();

        let full = vec![0.0f32; config.block_size];
        collector.buffer(&full).unwrap();

        let short = vec![0.0f32; 20];
        collector.buffer(&short).unwrap();
    }

    #[test]
    fn collector_rejects_variable_block_size_when_disabled() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, _analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model);
        collector.initialize(&config).unwrap();

        let full = vec![0.0f32; config.block_size];
        collector.buffer(&full).unwrap();

        let short = vec![0.0f32; 20];
        assert_eq!(collector.buffer(&short), Err(AicError::AudioConfigMismatch));
    }

    #[test]
    fn analyzer_reset_keeps_collector_initialized() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, mut analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model);
        collector.initialize(&config).unwrap();

        analyzer.reset().unwrap();

        let audio = vec![0.0f32; config.block_size];
        collector.buffer(&audio).unwrap();

        let result = analyzer.analyze_buffered().unwrap();
        assert_score_range(&result);
    }

    #[test]
    fn model_can_be_dropped_after_creating_analyzer_pair() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model);
        let (mut collector, mut analyzer) = test_analyzer_pair(&model, &license_key);
        drop(model); // The SDK keeps the model data alive for analyzer instances created from files.

        collector.initialize(&config).unwrap();

        let audio = vec![0.0f32; config.block_size];
        collector.buffer(&audio).unwrap();

        let result = analyzer.analyze_buffered().unwrap();
        assert_score_range(&result);
    }

    #[test]
    fn collector_and_analyzer_are_send_and_sync() {
        // Compile-time check that Collector and Analyzer can cross thread boundaries.
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Send>() {}

        assert_send::<Collector>();
        assert_sync::<Collector>();
        assert_send::<Analyzer>();
        assert_sync::<Analyzer>();
    }
}

#[doc(hidden)]
mod _compile_fail_tests {
    //! Compile-fail regression: an `Analyzer`'s model buffer must not be dropped before the analyzer.
    //!
    //! ```rust,compile_fail
    //! use aic_sdk::{Model, ProcessorConfig, analyzer_pair};
    //!
    //! fn main() {
    //!     let buffer = vec![0u8; 64];
    //!     let model = Model::from_buffer(&buffer).unwrap();
    //!     let config = ProcessorConfig::optimal(&model);
    //!
    //!     let (mut collector, mut analyzer) = analyzer_pair(&model, "license").unwrap();
    //!     collector.initialize(&config).unwrap();
    //!
    //!     drop(model); // Model can be dropped without issues
    //!
    //!     drop(buffer); // This should fail to compile
    //!
    //!     let audio = vec![0.0f32; config.block_size];
    //!     collector.buffer(&audio).unwrap();
    //!     analyzer.analyze_buffered().unwrap();
    //! }
    //! ```
}
