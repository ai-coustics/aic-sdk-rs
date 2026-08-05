use crate::{
    error::*,
    model::Model,
    processor::{OtelConfig, ProcessorConfig},
};

use aic_sdk_sys::{AicVadParameter::*, *};

use std::{ffi::CString, marker::PhantomData, ptr};

/// Configurable parameters for Voice Activity Detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VadParameter {
    /// Controls for how long the VAD continues to detect speech after the audio signal
    /// no longer contains speech.
    ///
    /// This affects the stability of speech detected -> not detected transitions.
    ///
    /// The VAD reports speech detected if the audio signal contained speech in at least 50%
    /// of the blocks processed in the last `speech_hold_duration * 2` seconds.
    ///
    /// For example, if `speech_hold_duration` is set to 0.5 seconds and the VAD stops detecting speech
    /// in the audio signal, the VAD will continue to report speech for 0.5 seconds assuming the
    /// VAD does not detect speech again during that period. If a few blocks of speech are detected
    /// during that period, those blocks will be included in the 50% calculation, which will extend
    /// the speech detection period until the 50% threshold is no longer met.
    ///
    /// NOTE: The VAD returns a value per processed audio block, so this duration is rounded
    /// to the closest model window length. For example, if the model has a processing window
    /// length of 10 ms, the VAD will round up/down to the closest multiple of 10 ms.
    /// Because of this, this parameter may return a different value than the one it was last set to.
    ///
    /// **Range:** 0.0 to 300x model window length (value in seconds)
    ///
    /// **Default:** model-specific
    SpeechHoldDuration,
    /// Controls the sensitivity of the VAD.
    ///
    /// VAD models output a probability of speech presence for each processed audio block,
    /// 1.0 being the model is certain speech is present and 0.0 being the model is certain
    /// speech is not present. The probability is compared against the sensitivity threshold
    /// to determine if speech is detected.
    ///
    /// A value above the threshold will trigger a speech detected decision.
    ///
    /// **Range:** 0.0 to 1.0
    ///
    /// **Default:** model-specific
    Sensitivity,
    /// Controls for how long speech needs to be present in the audio signal before
    /// the VAD considers it speech.
    ///
    /// This affects the stability of speech not detected -> detected transitions.
    ///
    /// NOTE: The VAD returns a value per processed audio block, so this duration is rounded
    /// to the closest model window length. For example, if the model has a processing window
    /// length of 10 ms, the VAD will round up/down to the closest multiple of 10 ms.
    /// Because of this, this parameter may return a different value than the one it was last set to.
    ///
    /// **Range:** 0.0 to 1.0 (value in seconds)
    ///
    /// **Default:** model-specific
    MinimumSpeechDuration,
}

impl From<VadParameter> for AicVadParameter::Type {
    fn from(parameter: VadParameter) -> Self {
        match parameter {
            VadParameter::SpeechHoldDuration => AIC_VAD_PARAMETER_SPEECH_HOLD_DURATION,
            VadParameter::Sensitivity => AIC_VAD_PARAMETER_SENSITIVITY,
            VadParameter::MinimumSpeechDuration => AIC_VAD_PARAMETER_MINIMUM_SPEECH_DURATION,
        }
    }
}

/// High-level wrapper for the ai-coustics voice activity detector.
///
/// A `Vad` is created from a VAD model (e.g. `vad-2.1-xxs-16khz`). Enhancement models
/// cannot be used for voice activity detection; pass them to a [`Processor`](crate::Processor)
/// instead.
///
/// Feed the audio to be examined to [`Vad::process`]. The audio is not modified, it only
/// updates the detector's prediction, which is read through a [`VadContext`].
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Model, ProcessorConfig, Vad};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/vad_model.aicmodel")?;
/// let config = ProcessorConfig::optimal(&model);
///
/// let mut vad = Vad::new(&model, &license_key)?.with_config(&config)?;
/// let vad_ctx = vad.context();
///
/// let audio_block = vec![0.0f32; config.block_size];
/// vad.process(&audio_block)?;
///
/// if vad_ctx.is_speech_detected() {
///     println!("Speech detected!");
/// }
/// # Ok::<(), aic_sdk::AicError>(())
/// ```
pub struct Vad<'a> {
    /// Raw pointer to the C VAD structure
    inner: *mut AicVad,
    /// Whether `initialize` has been called
    initialized: bool,
    /// Marker to tie the lifetime of the VAD to the lifetime of the model's weights
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Vad<'a> {
    /// Creates a new voice activity detector instance.
    ///
    /// Multiple VAD instances can be created to process different audio streams simultaneously.
    ///
    /// The same [`Model`] may be passed to this function more than once: each call creates an
    /// independent VAD that shares the underlying model data internally.
    ///
    /// # Arguments
    ///
    /// * `model` - The loaded model instance. Must be a VAD model, otherwise
    ///   [`AicError::ModelTypeUnsupported`] is returned.
    /// * `license_key` - license key for the ai-coustics SDK
    ///   (generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com/))
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Vad` instance or an [`AicError`] if creation fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Vad};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// let vad = Vad::new(&model, &license_key)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn new(model: &Model<'a>, license_key: &str) -> Result<Self, AicError> {
        Self::create(model, license_key, None)
    }

    /// Creates a new voice activity detector instance with explicit OpenTelemetry configuration.
    ///
    /// If provided, telemetry will be sent according to the provided configuration. Otherwise
    /// it will be configured according to the runtime environment.
    ///
    /// This overrides the SDK's environment-based telemetry defaults (e.g.
    /// `AIC_SDK_OTEL_ENABLE`) for this VAD.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, OtelConfig, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// let otel = OtelConfig::enabled();
    ///
    /// let vad = Vad::with_otel_config(&model, &license_key, &otel)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn with_otel_config(
        model: &Model<'a>,
        license_key: &str,
        otel_config: &OtelConfig,
    ) -> Result<Self, AicError> {
        Self::create(model, license_key, Some(otel_config))
    }

    fn create(
        model: &Model<'a>,
        license_key: &str,
        otel_config: Option<&OtelConfig>,
    ) -> Result<Self, AicError> {
        // Set the wrapper ID as soon as the user attempts to instantiate a VAD.
        // SAFETY: `2` is the wrapper ID assigned to this Rust SDK.
        unsafe { crate::set_sdk_id(2) };

        // Session ID must outlive the FFI call so its pointer stays valid.
        let c_session_id = otel_config
            .and_then(|o| o.session_id.as_deref())
            .map(CString::new)
            .transpose()
            .map_err(|_| AicError::Internal)?;

        let c_otel = otel_config.map(|o| AicOtelConfig {
            enable: o.enable,
            session_id: c_session_id.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
            export_interval_ms: o.export_interval_ms,
        });
        let c_otel_ptr = c_otel
            .as_ref()
            .map_or(ptr::null(), |o| o as *const AicOtelConfig);

        let mut vad_ptr: *mut AicVad = ptr::null_mut();
        let c_license_key =
            CString::new(license_key).map_err(|_| AicError::LicenseFormatInvalid)?;

        // SAFETY:
        // - `vad_ptr` points to stack storage for output.
        // - `model` is a valid SDK model pointer for the duration of the call.
        // - `c_license_key` is a null-terminated CString.
        // - `c_otel_ptr` is either null or points to a valid `AicOtelConfig` whose
        //   `session_id` field (if non-null) outlives this call.
        // - The output pointer is local to this call and not aliased.
        let error_code = unsafe {
            aic_vad_create(
                &mut vad_ptr,
                model.as_const_ptr(),
                c_license_key.as_ptr(),
                c_otel_ptr,
            )
        };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !vad_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: vad_ptr,
            initialized: false,
            marker: PhantomData,
        })
    }

    /// Initializes the VAD with the given configuration.
    ///
    /// This is a convenience method that calls [`Vad::initialize`] internally and returns `self`.
    /// The VAD is immediately ready to process audio after calling this method, so you don't
    /// need to call [`Vad::initialize`] separately.
    ///
    /// # Arguments
    ///
    /// * `config` - Audio processing configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` with the initialized VAD, or an [`AicError`] if initialization fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig, Vad};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// let config = ProcessorConfig::optimal(&model);
    ///
    /// let mut vad = Vad::new(&model, &license_key)?.with_config(&config)?;
    ///
    /// // VAD is ready to use - no need to call initialize()
    /// let audio_block = vec![0.0f32; config.block_size];
    /// vad.process(&audio_block)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn with_config(mut self, config: &ProcessorConfig) -> Result<Self, AicError> {
        self.initialize(config)?;
        Ok(self)
    }

    /// Configures the VAD for specific audio settings.
    ///
    /// This function must be called before processing any audio.
    /// For the most frequent prediction updates, use the sample rate and block size returned by
    /// [`Model::optimal_sample_rate`] and [`Model::optimal_block_size`].
    ///
    /// # Arguments
    ///
    /// * `config` - Audio processing configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if initialization fails.
    ///
    /// # Warning
    /// Do not call from audio processing threads as this allocates memory.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// # let mut vad = Vad::new(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model);
    /// vad.initialize(&config)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn initialize(&mut self, config: &ProcessorConfig) -> Result<(), AicError> {
        // SAFETY:
        // - `self.inner` is a valid pointer to a live VAD.
        // - This function is not thread-safe, so we borrow `&mut self`.
        let error_code = unsafe {
            aic_vad_initialize(
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

    /// Processes mono audio and updates the VAD prediction.
    ///
    /// This function does not modify the input audio buffer. Read the prediction through a
    /// [`VadContext`].
    ///
    /// # Recommendation
    ///
    /// When enhancement and VAD run together, pass the original input audio here, not the output
    /// of [`Processor::process`](crate::Processor::process). Enhancement is designed to change the
    /// signal, so running the VAD on its output means detecting speech in audio that no longer
    /// matches what the VAD model expects, and it stacks the processor's audio delay on top of the
    /// VAD's prediction delay. Because this function does not modify its input, calling it on the
    /// same buffer before `Processor::process` is enough:
    ///
    /// ```rust,ignore
    /// vad.process(&audio)?; // reads the block, does not modify it
    /// processor.process(&mut audio)?; // enhances the block in-place
    /// ```
    ///
    /// # Arguments
    ///
    /// * `audio` - Mono audio block to examine. Must match `block_size` from initialization, or
    ///   if `variable_block_size` was enabled, must be less than or equal to `block_size`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if processing fails.
    ///
    /// # Real-time safety
    ///
    /// Real-time safe. Can be called from audio processing threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// # let mut vad = Vad::new(&model, &license_key)?;
    /// let config = ProcessorConfig::optimal(&model);
    /// vad.initialize(&config)?;
    /// let audio = vec![0.0f32; config.block_size];
    /// vad.process(&audio)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn process(&mut self, audio: &[f32]) -> Result<(), AicError> {
        if !self.initialized {
            return Err(AicError::NotInitialized);
        }

        let audio_len = audio.len();

        // SAFETY:
        // - `self.inner` is a valid pointer to a live VAD.
        // - `audio` points to a contiguous, readable f32 slice of length `audio_len` that the
        //   C library only reads from.
        // - This function is not thread-safe, so we borrow `&mut self`.
        let error_code = unsafe { aic_vad_process(self.inner, audio.as_ptr(), audio_len) };

        handle_error(error_code)
    }

    /// Creates a [`VadContext`] instance.
    /// This can be used to read the prediction and to control all parameters and other
    /// settings of the VAD.
    ///
    /// All handles created from a given VAD reference the same VAD instance.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Vad};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// let vad = Vad::new(&model, &license_key)?;
    /// let vad_ctx = vad.context();
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn context(&self) -> VadContext {
        let mut context_ptr: *mut AicVadContext = ptr::null_mut();

        // SAFETY:
        // - `context_ptr` is valid output storage and not aliased.
        // - `self.as_const_ptr()` is a live VAD pointer.
        // - This function can be called from any thread and may run while the
        //   VAD is in use, so we only borrow `&self`.
        let error_code = unsafe { aic_vad_context_create(&mut context_ptr, self.as_const_ptr()) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !context_ptr.is_null(),
            "C library returned success but null pointer"
        );

        VadContext::new(context_ptr)
    }

    /// Terminates the telemetry session associated with this VAD.
    ///
    /// Once the request has been handled, the VAD is no longer allowed to process audio.
    ///
    /// This function is meant to be used in lifecycle management events.
    /// A telemetry session is automatically stopped when a VAD is destroyed.
    ///
    /// However, in cases where this SDK is integrated with languages with automatic memory
    /// management, object deallocation could be delayed. Use this function to start
    /// termination on demand.
    ///
    /// This function blocks until the telemetry session is terminated, unless another
    /// session is still alive. In that case, this function returns early and termination
    /// happens asynchronously. This keeps lifecycle management smooth while ensuring
    /// all sessions are closed when the last VAD is terminated.
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
    /// # use aic_sdk::{Model, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// let mut vad = Vad::new(&model, &license_key)?;
    /// vad.terminate_session()?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn terminate_session(&mut self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.inner` is a valid pointer to a live VAD.
        // - This function must not run concurrently with any other call taking the same
        //   VAD handle, so we borrow `&mut self`.
        let error_code = unsafe { aic_vad_terminate_session(self.inner) };
        handle_error(error_code)
    }

    fn as_const_ptr(&self) -> *const AicVad {
        self.inner as *const AicVad
    }
}

impl<'a> Drop for Vad<'a> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            // - This function is not thread-safe with concurrent VAD use, but
            //   `drop` has exclusive access to `self`.
            unsafe { aic_vad_destroy(self.inner) };
        }
    }
}

// SAFETY: Everything in Vad is Send, with the exception of the inner raw pointer.
// The Vad only uses the raw pointer according to the safety contracts of the
// unsafe APIs that require the pointer, and the Vad does not expose access to the
// raw pointer in any of its methods. Therefore, it is safe to implement Send for Vad.
unsafe impl<'a> Send for Vad<'a> {}

// SAFETY: Vad does not expose any interior mutability. The SDK functions that are documented
// as not thread-safe (`aic_vad_initialize`, `aic_vad_process`, `aic_vad_terminate_session`,
// `aic_vad_destroy`) are only reachable through methods that take `&mut self` or through `drop`,
// so Rust's borrow rules serialize them. The only method that takes `&self` (`context`) just
// creates a new context handle from a const VAD pointer, which is safe to do while the VAD is in
// use on another thread. Therefore, it is safe to implement Sync for Vad.
unsafe impl<'a> Sync for Vad<'a> {}

/// Thread-safe control handle for a [`Vad`].
///
/// Create one with [`Vad::context`]. Every method on this type maps to an SDK function that
/// can be called from any thread, so a context can be moved to another thread to read the
/// prediction, read and write parameters, query the prediction delay, or reset the VAD while audio is
/// being processed elsewhere.
///
/// All handles created from a given VAD reference the same VAD instance.
///
/// **Important:** If the backing [`Vad`] is dropped, the VAD stops producing new data. Dropping
/// the context does not destroy the VAD.
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Model, Vad};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/vad_model.aicmodel")?;
/// let vad = Vad::new(&model, &license_key)?;
/// let vad_ctx = vad.context();
/// # Ok::<(), aic_sdk::AicError>(())
/// ```
pub struct VadContext {
    /// Raw pointer to the C VAD context structure
    inner: *mut AicVadContext,
}

impl VadContext {
    /// Creates a new VAD context.
    pub(crate) fn new(context_ptr: *mut AicVadContext) -> Self {
        Self { inner: context_ptr }
    }

    fn as_const_ptr(&self) -> *const AicVadContext {
        self.inner as *const AicVadContext
    }

    /// Returns the VAD's prediction.
    ///
    /// # Latency
    ///
    /// The latency of the VAD prediction is equal to the backing VAD's processing latency,
    /// reported by [`VadContext::prediction_delay`]. The prediction lags its input by that many
    /// samples.
    ///
    /// Align speech decisions to the input timeline using that delay.
    ///
    /// If the backing VAD stops being processed, the VAD will not update its prediction.
    pub fn is_speech_detected(&self) -> bool {
        let mut value: bool = false;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD context.
        // - `value` points to stack storage for output.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code =
            unsafe { aic_vad_context_is_speech_detected(self.as_const_ptr(), &mut value) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());
        value
    }

    /// Returns the raw prediction of the VAD, without any processing.
    ///
    /// In contrast to the output of [`VadContext::is_speech_detected`],
    /// the output of this function is the model's direct prediction without
    /// going through the SDK's VAD post-processing (i.e. speech hold duration,
    /// sensitivity thresholding, etc.).
    ///
    /// This value may be used to build other abstractions on top of this data.
    ///
    /// # Latency
    ///
    /// The latency of the VAD prediction is equal to the backing VAD's processing latency,
    /// reported by [`VadContext::prediction_delay`]. The prediction lags its input by that many
    /// samples.
    ///
    /// Align speech decisions to the input timeline using that delay.
    ///
    /// If the backing VAD stops being processed, the VAD will not update its prediction.
    pub fn raw_vad_probability(&self) -> f32 {
        let mut value: f32 = 0.0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD context.
        // - `value` points to stack storage for output.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code =
            unsafe { aic_vad_context_get_raw_vad_probability(self.as_const_ptr(), &mut value) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());
        value
    }

    /// Modifies a VAD parameter.
    ///
    /// All parameters can be changed during audio processing.
    /// This function can be called from any thread.
    ///
    /// # Arguments
    ///
    /// - `parameter` - Parameter to modify
    /// - `value` - New parameter value. See parameter documentation for ranges
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if the parameter cannot be set.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Vad, VadParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// # let vad = Vad::new(&model, &license_key)?;
    /// # let vad_ctx = vad.context();
    /// vad_ctx.set_parameter(VadParameter::SpeechHoldDuration, 0.08)?;
    /// vad_ctx.set_parameter(VadParameter::Sensitivity, 0.5)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn set_parameter(&self, parameter: VadParameter, value: f32) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a live VAD context pointer.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code =
            unsafe { aic_vad_context_set_parameter(self.as_const_ptr(), parameter.into(), value) };
        handle_error(error_code)
    }

    /// Retrieves the current value of a VAD parameter.
    ///
    /// This function can be called from any thread.
    ///
    /// # Arguments
    ///
    /// - `parameter` - Parameter to query
    ///
    /// # Returns
    ///
    /// Returns `Ok(value)` containing the current parameter value, or an `AicError` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Vad, VadParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// # let vad = Vad::new(&model, &license_key)?;
    /// # let vad_ctx = vad.context();
    /// let sensitivity = vad_ctx.parameter(VadParameter::Sensitivity)?;
    /// println!("Current sensitivity: {sensitivity}");
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn parameter(&self, parameter: VadParameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD context.
        // - `value` points to stack storage for output.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code = unsafe {
            aic_vad_context_get_parameter(self.as_const_ptr(), parameter.into(), &mut value)
        };
        handle_error(error_code)?;
        Ok(value)
    }

    /// Returns the total VAD prediction delay in samples for the current audio configuration.
    ///
    /// This function provides the complete end-to-end latency of the VAD prediction, which
    /// includes input reblocking, STFT, and model processing delay. Use this value to line up
    /// VAD decisions with the input timeline.
    ///
    /// This delay is **not** applied to the audio: [`Vad::process`] leaves its input buffer
    /// untouched. The value only describes how far behind its input the published prediction is.
    ///
    /// When enhancement and VAD run together, feed the VAD the original input audio rather than
    /// the processor's output. This value is then the prediction's delay relative to that input,
    /// and it is independent of the processor's
    /// [`ProcessorContext::audio_delay`](crate::ProcessorContext::audio_delay).
    ///
    /// **Delay behavior:**
    /// - **Before initialization:** Returns the base processing delay using the model's
    ///   optimal block size at its native sample rate
    /// - **After initialization:** Returns the end-to-end VAD prediction delay at the
    ///   initialized sample rate, including the input-buffering latency of the configured
    ///   block size
    ///
    /// **Important:** The delay value is always expressed in samples at the sample rate
    /// you configured during [`Vad::initialize`]. To convert to time units:
    /// `delay_ms = (delay_samples * 1000) / sample_rate`
    ///
    /// **Note:** Using a block size different from the optimal value returned by
    /// [`Model::optimal_block_size`], or enabling variable block sizes, can add input-buffering
    /// latency before a new VAD prediction is published. That latency is included in the
    /// reported delay.
    ///
    /// # Returns
    ///
    /// Returns the delay in samples.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// # let vad = Vad::new(&model, &license_key)?;
    /// # let vad_ctx = vad.context();
    /// let delay = vad_ctx.prediction_delay();
    /// println!("VAD prediction delay: {delay} samples");
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn prediction_delay(&self) -> usize {
        let mut delay: usize = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD context.
        // - `delay` points to stack storage for output.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code =
            unsafe { aic_vad_context_get_prediction_delay(self.as_const_ptr(), &mut delay) };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_vad_context_get_prediction_delay` is documented to always succeed if given
        // valid pointers.
        assert_success(
            error_code,
            "`aic_vad_context_get_prediction_delay` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        delay
    }

    /// Clears all internal state and buffers. This also resets the VAD state, so the published
    /// speech detection and raw probability values are cleared immediately.
    ///
    /// Call this when the audio stream is interrupted or when seeking
    /// to prevent mispredictions from previous audio content.
    ///
    /// The VAD stays initialized to the configured settings.
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
    /// # use aic_sdk::{Model, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// # let vad = Vad::new(&model, &license_key)?;
    /// # let vad_ctx = vad.context();
    /// vad_ctx.reset()?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn reset(&self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD context.
        // - This function can be called from any thread, so we only borrow `&self`.
        let error_code = unsafe { aic_vad_context_reset(self.as_const_ptr()) };
        handle_error(error_code)
    }

    /// Replaces the bearer token on the running VAD.
    ///
    /// Use this when your license key is a JWT and needs to be refreshed
    /// before it expires. Calling this with a renewed token lets you stay authenticated
    /// without tearing down and recreating the VAD: audio processing continues
    /// uninterrupted, the context handle stays valid, and the new token is used for all
    /// subsequent authentication against the ai-coustics backend.
    ///
    /// In-place updates are only supported when both the originally configured key and the
    /// new token are JWTs. Other license types cannot be swapped in this way.
    ///
    /// On any error the call is a no-op: the previously active token stays in use and the
    /// telemetry session is unaffected (no backoff, no interruption to processing).
    ///
    /// On success the swap is applied immediately and is **not** gated on backend
    /// acceptance. The token is validated locally for format only; if the backend later
    /// rejects it (e.g. expired or revoked), the SDK retries it under backoff rather than
    /// rolling back to the prior token, and audio processing is eventually disabled if no
    /// accepted token arrives in time. Supplying a known-good token via this call during
    /// that window recovers the session.
    ///
    /// Safe to call concurrently with [`Vad::process`] on the originating VAD.
    ///
    /// # Arguments
    ///
    /// * `token` - The new JWT to install.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an [`AicError`] if the update fails.
    ///
    /// # Real-time safety
    ///
    /// This function is not real-time safe. It locks a mutex and allocates memory.
    /// Avoid calling it from audio threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Vad};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/vad_model.aicmodel")?;
    /// let vad = Vad::new(&model, &license_key)?;
    /// let vad_ctx = vad.context();
    /// let renewed_jwt = String::from("<JWT_BEARER_TOKEN>");
    /// vad_ctx.update_bearer_token(&renewed_jwt)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn update_bearer_token(&self, token: &str) -> Result<(), AicError> {
        let c_token = CString::new(token).map_err(|_| AicError::LicenseFormatInvalid)?;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD context.
        // - `c_token` is a null-terminated CString that outlives the call.
        // - This function can be called from any thread.
        let error_code =
            unsafe { aic_vad_context_update_bearer_token(self.as_const_ptr(), c_token.as_ptr()) };
        handle_error(error_code)
    }
}

impl Drop for VadContext {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            // - This function can be called from any thread; `drop` has exclusive
            //   access to this VAD context handle.
            unsafe { aic_vad_context_destroy(self.inner) };
        }
    }
}

// Safety: The underlying C library should be thread-safe for individual VadContext instances
unsafe impl Send for VadContext {}
unsafe impl Sync for VadContext {}

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

    fn find_existing_model(target_dir: &Path, name_fragment: &str) -> Option<PathBuf> {
        let entries = fs::read_dir(target_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.contains(name_fragment) && name.ends_with(".aicmodel"))
                .unwrap_or(false)
                && path.is_file()
            {
                return Some(path);
            }
        }
        None
    }

    /// Downloads `model_id` into the crate's `target/` directory and returns its path.
    fn get_model(model_id: &str, name_fragment: &str) -> Result<PathBuf, AicError> {
        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");

        if let Some(existing) = find_existing_model(&target_dir, name_fragment) {
            return Ok(existing);
        }

        let _guard = download_lock().lock().unwrap();
        if let Some(existing) = find_existing_model(&target_dir, name_fragment) {
            return Ok(existing);
        }

        if cfg!(feature = "download-model") {
            Model::download(model_id, target_dir)
        } else {
            panic!(
                "Model `{model_id}` not found in {} and `download-model` feature is disabled",
                target_dir.display()
            );
        }
    }

    fn license_key() -> String {
        std::env::var("AIC_SDK_LICENSE")
            .expect("AIC_SDK_LICENSE environment variable must be set for tests")
    }

    fn load_vad_model() -> Model<'static> {
        let model_path = get_model("vad-2.1-xxs-16khz", "vad_2_1_xxs_16khz").unwrap();
        Model::from_file(&model_path).unwrap()
    }

    #[test]
    fn vad_processes_audio_and_reports_prediction() {
        let model = load_vad_model();
        let config = ProcessorConfig::optimal(&model);

        let mut vad = Vad::new(&model, &license_key())
            .unwrap()
            .with_config(&config)
            .unwrap();

        let vad_ctx = vad.context();
        assert!(vad_ctx.prediction_delay() > 0);

        let audio = vec![0.0f32; config.block_size];
        vad.process(&audio).unwrap();

        // Silence must not be reported as speech.
        assert!(!vad_ctx.is_speech_detected());
        assert!((0.0..=1.0).contains(&vad_ctx.raw_vad_probability()));

        vad_ctx.reset().unwrap();
    }

    #[test]
    fn vad_rejects_process_before_initialize() {
        let model = load_vad_model();
        let mut vad = Vad::new(&model, &license_key()).unwrap();

        let audio = vec![0.0f32; 160];
        assert_eq!(vad.process(&audio), Err(AicError::NotInitialized));
    }

    #[test]
    fn vad_rejects_enhancement_model() {
        let model_path = get_model("rook-s-48khz", "rook_s_48khz").unwrap();
        let model = Model::from_file(&model_path).unwrap();

        assert_eq!(
            Vad::new(&model, &license_key()).err(),
            Some(AicError::ModelTypeUnsupported)
        );
    }

    #[test]
    fn vad_parameters_round_trip() {
        let model = load_vad_model();
        let vad = Vad::new(&model, &license_key()).unwrap();
        let vad_ctx = vad.context();

        vad_ctx
            .set_parameter(VadParameter::Sensitivity, 0.5)
            .unwrap();
        assert_eq!(vad_ctx.parameter(VadParameter::Sensitivity).unwrap(), 0.5);

        // The sensitivity of a VAD model is a probability threshold.
        assert_eq!(
            vad_ctx.set_parameter(VadParameter::Sensitivity, 7.0),
            Err(AicError::ParameterOutOfRange)
        );
    }

    #[test]
    fn vad_is_send_and_sync() {
        // Compile-time check that Vad and VadContext implement Send and Sync.
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Vad>();
        assert_sync::<Vad>();
        assert_send::<VadContext>();
        assert_sync::<VadContext>();
    }
}

#[doc(hidden)]
mod _compile_fail_tests {
    //! Compile-fail regression: a `Vad`'s model buffer must not be dropped before the VAD.
    //!
    //! ```rust,compile_fail
    //! use aic_sdk::{Model, ProcessorConfig, Vad};
    //!
    //! fn main() {
    //!     let buffer = vec![0u8; 64];
    //!     let model = Model::from_buffer(&buffer).unwrap();
    //!     let config = ProcessorConfig::optimal(&model);
    //!
    //!     let mut vad = Vad::new(&model, "license")
    //!         .unwrap()
    //!         .with_config(&config)
    //!         .unwrap();
    //!
    //!     drop(model); // Model can be dropped without issues
    //!
    //!     drop(buffer); // This should fail to compile
    //!
    //!     let audio = vec![0.0f32; config.block_size];
    //!     vad.process(&audio).unwrap();
    //! }
    //! ```
}
