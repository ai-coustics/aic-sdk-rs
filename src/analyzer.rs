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
/// * `model` - The loaded model instance
/// * `license_key` - license key for the ai-coustics SDK
///   (generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com/))
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
    /// Configured number of channels.
    num_channels: Option<u16>,
}

impl Collector {
    fn new(collector_ptr: *mut AicCollector) -> Self {
        Self {
            inner: collector_ptr,
            num_channels: None,
        }
    }

    /// Configures the collector for specific audio settings.
    ///
    /// This function must be called before buffering any audio.
    /// For the lowest delay use the sample rate and frame size returned by
    /// [`Model::optimal_sample_rate`] and [`Model::optimal_num_frames`].
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
    /// # Note
    /// All channels are mixed to mono for buffering. To buffer channels
    /// independently, create separate [`Collector`] instances.
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
        let error_code = unsafe {
            aic_collector_initialize(
                self.inner,
                config.sample_rate,
                config.num_channels,
                config.num_frames,
                config.allow_variable_frames,
            )
        };

        handle_error(error_code)?;
        self.num_channels = Some(config.num_channels);
        Ok(())
    }

    /// Buffers audio with separate buffers for each channel (planar layout).
    ///
    /// **Memory Layout:**
    /// - Separate buffer for each channel
    /// - Each buffer contains `num_frames` floats
    /// - Maximum of 16 channels supported
    /// - Example for 2 channels, 4 frames:
    ///   ```text
    ///   audio[0] -> [ch0_f0, ch0_f1, ch0_f2, ch0_f3]
    ///   audio[1] -> [ch1_f0, ch1_f1, ch1_f2, ch1_f3]
    ///   ```
    ///
    /// The function accepts any type of collection of `f32` values that implements `as_mut`, e.g.:
    /// - `[vec![0.0; 128]; 2]`
    /// - `[[0.0; 128]; 2]`
    /// - `[&mut ch1, &mut ch2]`
    ///
    /// # Arguments
    ///
    /// * `audio` - Array of mutable channel buffer slices to be buffered.
    ///             Each channel buffer must be exactly of size `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value.
    ///
    /// # Notes
    ///
    /// - All channels are mixed to mono for buffering. To buffer channels
    ///   independently, create separate [`Collector`] instances.
    /// - Maximum supported number of channels is 16. Exceeding this will return an error.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if buffering fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (mut collector, _) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    /// collector.initialize(&config)?;
    /// let audio = vec![vec![0.0f32; config.num_frames]; config.num_channels as usize];
    /// collector.buffer_planar(&audio)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn buffer_planar<V: AsRef<[f32]>>(&mut self, audio: &[V]) -> Result<(), AicError> {
        const MAX_CHANNELS: u16 = 16;

        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ProcessorNotInitialized);
        };

        if audio.len() != num_channels as usize {
            return Err(AicError::AudioConfigMismatch);
        }

        if num_channels > MAX_CHANNELS {
            return Err(AicError::AudioConfigUnsupported);
        }

        let num_frames = if audio.is_empty() {
            0
        } else {
            audio[0].as_ref().len()
        };

        let mut audio_ptrs = [std::ptr::null::<f32>(); MAX_CHANNELS as usize];
        for (i, channel) in audio.iter().enumerate() {
            if channel.as_ref().len() != num_frames {
                return Err(AicError::AudioConfigMismatch);
            }
            audio_ptrs[i] = channel.as_ref().as_ptr();
        }

        // SAFETY:
        // - `self.inner` is a valid pointer to a live collector.
        // - `audio_ptrs` holds `num_channels` valid readable pointers with `num_frames` samples each.
        let error_code = unsafe {
            aic_collector_buffer_planar(self.inner, audio_ptrs.as_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }

    /// Buffers audio with interleaved channel data.
    ///
    /// **Memory Layout:**
    /// - Single contiguous buffer with samples alternating between channels
    /// - Buffer size: `num_channels` * `num_frames` floats
    /// - Example for 2 channels, 4 frames:
    ///   ```text
    ///   audio -> [ch0_f0, ch1_f0, ch0_f1, ch1_f1, ch0_f2, ch1_f2, ch0_f3, ch1_f3]
    ///   ```
    ///
    /// # Arguments
    ///
    /// * `audio` - Interleaved audio buffer to be buffered.
    ///             Must be exactly of size `num_channels` * `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value per channel.
    ///
    /// # Note
    ///
    /// All channels are mixed to mono for buffering. To buffer channels
    /// independently, create separate [`Collector`] instances.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if buffering fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (mut collector, _) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    /// collector.initialize(&config)?;
    /// let audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    /// collector.buffer_interleaved(&audio)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn buffer_interleaved(&mut self, audio: &[f32]) -> Result<(), AicError> {
        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ProcessorNotInitialized);
        };

        if !audio.len().is_multiple_of(num_channels as usize) {
            return Err(AicError::AudioConfigMismatch);
        }

        let num_frames = audio.len() / num_channels as usize;

        // SAFETY:
        // - `self.inner` is a valid pointer to a live collector.
        // - `audio` points to a contiguous f32 slice of length `num_channels * num_frames`.
        let error_code = unsafe {
            aic_collector_buffer_interleaved(self.inner, audio.as_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }

    /// Buffers audio with sequential channel data.
    ///
    /// **Memory Layout:**
    /// - Single contiguous buffer with all samples for each channel stored sequentially
    /// - Buffer size: `num_channels` * `num_frames` floats
    /// - Example for 2 channels, 4 frames:
    ///   ```text
    ///   audio -> [ch0_f0, ch0_f1, ch0_f2, ch0_f3, ch1_f0, ch1_f1, ch1_f2, ch1_f3]
    ///   ```
    ///
    /// # Arguments
    ///
    /// * `audio` - Sequential audio buffer to be buffered.
    ///             Must be exactly of size `num_channels` * `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value per channel.
    /// # Note
    ///
    /// All channels are mixed to mono for buffering. To buffer channels
    /// independently, create separate [`Collector`] instances.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if buffering fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// # let (mut collector, _) = aic_sdk::analyzer_pair(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    /// collector.initialize(&config)?;
    /// let audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    /// collector.buffer_sequential(&audio)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn buffer_sequential(&mut self, audio: &[f32]) -> Result<(), AicError> {
        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ProcessorNotInitialized);
        };

        if !audio.len().is_multiple_of(num_channels as usize) {
            return Err(AicError::AudioConfigMismatch);
        }

        let num_frames = audio.len() / num_channels as usize;

        // SAFETY:
        // - `self.inner` is a valid pointer to a live collector.
        // - `audio` points to a contiguous f32 slice of length `num_channels * num_frames`.
        let error_code = unsafe {
            aic_collector_buffer_sequential(self.inner, audio.as_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_collector_destroy(self.inner) };
        }
    }
}

// SAFETY: Everything in Collector is Send, with the exception of the inner raw pointer.
// The Collector only uses the raw pointer according to the safety contracts of the
// unsafe APIs that require the pointer, and the Collector does not expose access to the
// raw pointer in any of its methods. Therefore, it safe to implement Send for Collector.
unsafe impl Send for Collector {}

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
    /// # Safety
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
    /// # Note
    /// When buffering, all channels are mixed down to mono. To analyze channels
    /// independently, create separate analyzer pairs.
    ///
    /// # Returns
    ///
    /// Returns an [`AnalysisResult`] if successful, otherwise an [`AicError`].
    ///
    /// # Safety
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
        let error_code = unsafe { aic_analyzer_analyze_buffered(self.inner, &mut result) };
        handle_error(error_code)?;

        Ok(result.into())
    }

    /// Replaces the bearer token on the analyzer.
    ///
    /// Use this when your license key is a JWT and needs to be refreshed before it expires.
    /// Audio processing continues uninterrupted, the context handle stays valid, and the new
    /// token is used for all subsequent authentication against the ai-coustics backend.
    ///
    /// In-place updates are only supported when both the originally configured key and the
    /// new token are JWTs. If either side is not, the call returns
    /// [`AicError::TokenUpdateUnsupported`] and the existing token stays in use.
    ///
    /// # Arguments
    ///
    /// * `token` - The new JWT to install.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if the update fails.
    ///
    /// # Safety
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
            unsafe { aic_analyzer_destroy(self.inner) };
        }
    }
}

// SAFETY: Everything in Analyzer is Send, with the exception of the inner raw pointer.
// The Analyzer only uses the raw pointer according to the safety contracts of the
// unsafe APIs that require the pointer, and the Analyzer does not expose access to the
// raw pointer in any of its methods. Therefore, it safe to implement Send for Analyzer.
unsafe impl<'a> Send for Analyzer<'a> {}

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
            num_channels: None,
        };

        let planar = [vec![0.0f32; 4]];
        let contiguous = vec![0.0f32; 4];

        assert_eq!(
            collector.buffer_planar(&planar),
            Err(AicError::ProcessorNotInitialized)
        );
        assert_eq!(
            collector.buffer_interleaved(&contiguous),
            Err(AicError::ProcessorNotInitialized)
        );
        assert_eq!(
            collector.buffer_sequential(&contiguous),
            Err(AicError::ProcessorNotInitialized)
        );
    }

    #[test]
    fn collector_validates_planar_layout_before_ffi() {
        let mut collector = Collector {
            inner: ptr::null_mut(),
            num_channels: Some(2),
        };

        let wrong_channel_count = [vec![0.0f32; 4]];
        assert_eq!(
            collector.buffer_planar(&wrong_channel_count),
            Err(AicError::AudioConfigMismatch)
        );

        let mismatched_frames = [vec![0.0f32; 4], vec![0.0f32; 3]];
        assert_eq!(
            collector.buffer_planar(&mismatched_frames),
            Err(AicError::AudioConfigMismatch)
        );

        collector.num_channels = Some(17);
        let too_many_channels = vec![vec![0.0f32; 4]; 17];
        assert_eq!(
            collector.buffer_planar(&too_many_channels),
            Err(AicError::AudioConfigUnsupported)
        );
    }

    #[test]
    fn collector_validates_contiguous_layout_before_ffi() {
        let mut collector = Collector {
            inner: ptr::null_mut(),
            num_channels: Some(2),
        };
        let not_divisible_by_channels = vec![0.0f32; 3];

        assert_eq!(
            collector.buffer_interleaved(&not_divisible_by_channels),
            Err(AicError::AudioConfigMismatch)
        );
        assert_eq!(
            collector.buffer_sequential(&not_divisible_by_channels),
            Err(AicError::AudioConfigMismatch)
        );
    }

    #[test]
    fn analyzer_pair_rejects_license_key_with_nul() {
        let (model, _) = load_test_model().unwrap();

        let result = analyzer_pair(&model, "invalid\0license");

        assert!(matches!(result, Err(AicError::LicenseFormatInvalid)));
    }

    #[test]
    fn collector_buffers_all_layouts_and_analyzer_returns_scores() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, mut analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);
        collector.initialize(&config).unwrap();

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let planar = [left.as_mut_slice(), right.as_mut_slice()];
        collector.buffer_planar(&planar).unwrap();

        let num_channels = config.num_channels as usize;
        let contiguous = vec![0.0f32; num_channels * config.num_frames];
        collector.buffer_interleaved(&contiguous).unwrap();
        collector.buffer_sequential(&contiguous).unwrap();

        let result = analyzer.analyze_buffered().unwrap();
        assert_score_range(&result);
    }

    #[test]
    fn collector_buffers_variable_frames_when_enabled() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, _analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model)
            .with_num_channels(2)
            .with_allow_variable_frames(true);
        collector.initialize(&config).unwrap();

        let num_channels = config.num_channels as usize;
        let full = vec![0.0f32; num_channels * config.num_frames];
        collector.buffer_interleaved(&full).unwrap();
        collector.buffer_sequential(&full).unwrap();

        let short = vec![0.0f32; num_channels * 20];
        collector.buffer_interleaved(&short).unwrap();
        collector.buffer_sequential(&short).unwrap();

        let left = vec![0.0f32; 20];
        let right = vec![0.0f32; 20];
        let planar = [left.as_slice(), right.as_slice()];
        collector.buffer_planar(&planar).unwrap();
    }

    #[test]
    fn collector_rejects_variable_frames_when_disabled() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, _analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);
        collector.initialize(&config).unwrap();

        let num_channels = config.num_channels as usize;
        let full = vec![0.0f32; num_channels * config.num_frames];
        collector.buffer_interleaved(&full).unwrap();
        collector.buffer_sequential(&full).unwrap();

        let short = vec![0.0f32; num_channels * 20];
        assert_eq!(
            collector.buffer_interleaved(&short),
            Err(AicError::AudioConfigMismatch)
        );
        assert_eq!(
            collector.buffer_sequential(&short),
            Err(AicError::AudioConfigMismatch)
        );

        let left = vec![0.0f32; 20];
        let right = vec![0.0f32; 20];
        let planar = [left.as_slice(), right.as_slice()];
        assert_eq!(
            collector.buffer_planar(&planar),
            Err(AicError::AudioConfigMismatch)
        );
    }

    #[test]
    fn analyzer_reset_keeps_collector_initialized() {
        let (model, license_key) = load_test_model().unwrap();
        let (mut collector, mut analyzer) = test_analyzer_pair(&model, &license_key);
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);
        collector.initialize(&config).unwrap();

        analyzer.reset().unwrap();

        let num_channels = config.num_channels as usize;
        let audio = vec![0.0f32; num_channels * config.num_frames];
        collector.buffer_interleaved(&audio).unwrap();

        let result = analyzer.analyze_buffered().unwrap();
        assert_score_range(&result);
    }

    #[test]
    fn model_can_be_dropped_after_creating_analyzer_pair() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);
        let (mut collector, mut analyzer) = test_analyzer_pair(&model, &license_key);
        drop(model); // The SDK keeps the model data alive for analyzer instances created from files.

        collector.initialize(&config).unwrap();

        let num_channels = config.num_channels as usize;
        let audio = vec![0.0f32; num_channels * config.num_frames];
        collector.buffer_interleaved(&audio).unwrap();

        let result = analyzer.analyze_buffered().unwrap();
        assert_score_range(&result);
    }

    #[test]
    fn collector_and_analyzer_are_send() {
        // Compile-time check that Collector and Analyzer can cross thread boundaries.
        fn assert_send<T: Send>() {}

        assert_send::<Collector>();
        assert_send::<Analyzer>();
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
    //!     let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    //!
    //!     let (mut collector, mut analyzer) = analyzer_pair(&model, "license").unwrap();
    //!     collector.initialize(&config).unwrap();
    //!
    //!     drop(model); // Model can be dropped without issues
    //!
    //!     drop(buffer); // This should fail to compile
    //!
    //!     let audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    //!     collector.buffer_interleaved(&audio).unwrap();
    //!     analyzer.analyze_buffered().unwrap();
    //! }
    //! ```
}
