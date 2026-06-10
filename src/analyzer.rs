use crate::{error::*, model::Model, processor::ProcessorConfig};

use aic_sdk_sys::*;

use std::{ffi::CString, marker::PhantomData, ptr};

/// The result of analyzing an audio signal with an [`Analyzer`].
///
/// Scores are in the range `0.0..=1.0`. For all fields except
/// [`speaker_loudness`](Self::speaker_loudness), lower values indicate less problematic audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioInsights {
    /// Headline audio score.
    ///
    /// Predicts likelihood of failure of downstream models including speech-to-text,
    /// voice activity detection, turn-taking, or speech-to-speech models.
    pub tyto_score: f32,
    /// Measure of speaker distance and reverberance.
    pub speaker_reverb: f32,
    /// Measure of speaker loudness.
    pub speaker_loudness: f32,
    /// Measure of interference from additional speakers present in audio.
    pub interfering_speech: f32,
    /// Measure of interfering speech content from media devices.
    pub media_speech: f32,
    /// Measure of ambient or environmental noise.
    pub noise: f32,
    /// Measure of audio dropouts or discontinuities in the stream.
    pub packet_loss: f32,
}

impl From<AicAudioInsights> for AudioInsights {
    fn from(insights: AicAudioInsights) -> Self {
        Self {
            tyto_score: insights.tyto_score,
            speaker_reverb: insights.speaker_reverb,
            speaker_loudness: insights.speaker_loudness,
            interfering_speech: insights.interfering_speech,
            media_speech: insights.media_speech,
            noise: insights.noise,
            packet_loss: insights.packet_loss,
        }
    }
}

/// A collector/analyzer pair for non-real-time audio analysis.
///
/// The [`Collector`] is intended for buffering audio from the audio thread, while
/// the [`Analyzer`] can run the more expensive analysis work elsewhere. The two handles
/// are independent and may be destroyed in any order.
pub struct AnalyzerPair<'a> {
    /// Buffers audio for later analysis.
    pub collector: Collector<'a>,
    /// Analyzes the audio buffered by the collector.
    pub analyzer: Analyzer<'a>,
}

impl<'a> AnalyzerPair<'a> {
    /// Creates a collector/analyzer pair.
    ///
    /// `analysis_window_length_ms` controls how much recent audio the collector retains
    /// for each analysis run. The accepted range is model-specific.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{AnalyzerPair, Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/analysis-model.aicmodel")?;
    /// let config = ProcessorConfig::optimal(&model);
    /// let mut pair = AnalyzerPair::new(&model, &license_key, 30_000)?;
    ///
    /// pair.collector.initialize(&config)?;
    /// let audio = vec![0.0f32; config.num_frames];
    /// pair.collector.buffer_interleaved(&audio)?;
    ///
    /// let insights = pair.analyzer.analyze_buffered()?;
    /// println!("Tyto score: {}", insights.tyto_score);
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn new(
        model: &Model<'a>,
        license_key: &str,
        analysis_window_length_ms: usize,
    ) -> Result<Self, AicError> {
        // Set the wrapper ID as soon as the user attempts to instantiate an analyzer
        crate::set_wrapper_id();

        let mut collector_ptr: *mut AicCollector = ptr::null_mut();
        let mut analyzer_ptr: *mut AicAnalyzer = ptr::null_mut();
        let c_license_key =
            CString::new(license_key).map_err(|_| AicError::LicenseFormatInvalid)?;

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
                analysis_window_length_ms,
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

        Ok(Self {
            collector: Collector::new(collector_ptr),
            analyzer: Analyzer::new(analyzer_ptr),
        })
    }

    /// Splits this pair into independently owned collector and analyzer handles.
    pub fn into_parts(self) -> (Collector<'a>, Analyzer<'a>) {
        (self.collector, self.analyzer)
    }
}

/// Buffers audio for later non-real-time analysis.
pub struct Collector<'a> {
    /// Raw pointer to the C collector structure.
    inner: *mut AicCollector,
    /// Configured number of channels.
    num_channels: Option<u16>,
    /// Marker to tie the collector to the lifetime of the model's weights.
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Collector<'a> {
    pub(crate) fn new(collector_ptr: *mut AicCollector) -> Self {
        Self {
            inner: collector_ptr,
            num_channels: None,
            marker: PhantomData,
        }
    }

    /// Initializes the collector with the given audio configuration.
    ///
    /// This must be called before buffering audio.
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

    /// Initializes the collector and returns it.
    pub fn with_config(mut self, config: &ProcessorConfig) -> Result<Self, AicError> {
        self.initialize(config)?;
        Ok(self)
    }

    /// Buffers audio with separate read-only buffers for each channel.
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

impl<'a> Drop for Collector<'a> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_collector_destroy(self.inner) };
        }
    }
}

unsafe impl<'a> Send for Collector<'a> {}
unsafe impl<'a> Sync for Collector<'a> {}

/// Runs non-real-time analysis over audio buffered by a [`Collector`].
pub struct Analyzer<'a> {
    /// Raw pointer to the C analyzer structure.
    inner: *mut AicAnalyzer,
    /// Marker to tie the analyzer to the lifetime of the model's weights.
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Analyzer<'a> {
    pub(crate) fn new(analyzer_ptr: *mut AicAnalyzer) -> Self {
        Self {
            inner: analyzer_ptr,
            marker: PhantomData,
        }
    }

    fn as_const_ptr(&self) -> *const AicAnalyzer {
        self.inner as *const AicAnalyzer
    }

    /// Clears the analyzer's internal state.
    pub fn reset(&self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live analyzer.
        let error_code = unsafe { aic_analyzer_reset(self.as_const_ptr()) };
        handle_error(error_code)
    }

    /// Analyzes the audio currently buffered by the paired collector.
    pub fn analyze_buffered(&mut self) -> Result<AudioInsights, AicError> {
        let mut insights = AicAudioInsights {
            tyto_score: 0.0,
            speaker_reverb: 0.0,
            speaker_loudness: 0.0,
            interfering_speech: 0.0,
            media_speech: 0.0,
            noise: 0.0,
            packet_loss: 0.0,
        };

        // SAFETY:
        // - `self.inner` is a valid pointer to a live analyzer.
        // - `insights` points to stack storage for output.
        let error_code = unsafe { aic_analyzer_analyze_buffered(self.inner, &mut insights) };
        handle_error(error_code)?;

        Ok(insights.into())
    }

    /// Replaces the bearer token on the running analyzer.
    ///
    /// Use this when your license key is a JWT and needs to be refreshed before it expires.
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

unsafe impl<'a> Send for Analyzer<'a> {}
unsafe impl<'a> Sync for Analyzer<'a> {}
