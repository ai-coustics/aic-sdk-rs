use crate::{error::*, model::Model};

use aic_sdk_sys::{AicProcessorParameter::*, *};

use std::{ffi::CString, marker::PhantomData, ptr, sync::Once};

/// Public for telemetry purposes
pub static SET_WRAPPER_ID: Once = Once::new();
pub use aic_sdk_sys::aic_set_sdk_wrapper_id;

/// Audio processing configuration passed to [`Processor::initialize`].
///
/// Use [`Model::optimal_processor_config`] as a starting point, then adjust fields
/// to match your stream layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessorConfig {
    /// Sample rate in Hz (8000 - 192000).
    pub sample_rate: u32,
    /// Number of audio channels in the stream (1 for mono, 2 for stereo, etc).
    pub num_channels: u16,
    /// Samples per channel provided to each processing call.
    /// Note that using a non-optimal number of frames increases latency.
    pub num_frames: usize,
    /// Allows frame counts below `num_frames` at the cost of added latency.
    pub allow_variable_frames: bool,
}

impl ProcessorConfig {
    /// Returns a [`ProcessorConfig`] pre-filled with the model's optimal sample rate and frame size.
    ///
    /// `num_channels` will be set to `1` and `allow_variable_frames` to `false`.
    /// Adjust the number of channels and enable variable frames by using the builder pattern.
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// let config = ProcessorConfig::optimal(&model)
    ///     .with_num_channels(2)
    ///     .with_allow_variable_frames(true);
    /// ```
    ///
    /// If you need to configure a non-optimal sample rate or number of frames,
    /// construct the [`ProcessorConfig`] struct directly. For example:
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let config = ProcessorConfig {
    ///     num_channels: 2,
    ///     sample_rate: 44100,
    ///     num_frames: model.optimal_num_frames(44100),
    ///     allow_variable_frames: true,
    /// };
    /// ```
    pub fn optimal(model: &Model) -> Self {
        let sample_rate = model.optimal_sample_rate();
        let num_frames = model.optimal_num_frames(sample_rate);
        ProcessorConfig {
            sample_rate,
            num_channels: 1,
            num_frames,
            allow_variable_frames: false,
        }
    }

    /// Sets the number of audio channels for processing.
    ///
    /// # Arguments
    ///
    /// * `num_channels` - Number of audio channels (1 for mono, 2 for stereo, etc.)
    pub fn with_num_channels(mut self, num_channels: u16) -> Self {
        self.num_channels = num_channels;
        self
    }

    /// Enables or disables variable frame size support.
    ///
    /// When enabled, allows processing frame counts below `num_frames` at the cost of added latency.
    ///
    /// # Arguments
    ///
    /// * `allow_variable_frames` - `true` to enable variable frame sizes, `false` for fixed size
    pub fn with_allow_variable_frames(mut self, allow_variable_frames: bool) -> Self {
        self.allow_variable_frames = allow_variable_frames;
        self
    }
}

/// Configurable parameters for audio enhancement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessorParameter {
    /// Controls whether audio processing is bypassed while preserving algorithmic delay.
    ///
    /// When enabled, the input audio passes through unmodified, but the output is still
    /// delayed by the same amount as during normal processing. This ensures seamless
    /// transitions when toggling enhancement on/off without audible clicks or timing shifts.
    ///
    /// **Range:** 0.0 to 1.0
    /// - **0.0:** Enhancement active (normal processing)
    /// - **1.0:** Bypass enabled (latency-compensated passthrough)
    ///
    /// **Default:** 0.0
    Bypass,
    /// Controls the intensity of speech enhancement processing.
    ///
    /// **Range:** 0.0 to 1.0
    /// - **0.0:** Bypass mode - original signal passes through unchanged
    /// - **1.0:** Full enhancement - maximum noise reduction but also more audible artifacts
    ///
    /// **Default:** 1.0
    EnhancementLevel,
    /// Compensates for perceived volume reduction after noise removal.
    ///
    /// **Range:** 0.1 to 4.0 (linear amplitude multiplier)
    /// - **0.1:** Significant volume reduction (-20 dB)
    /// - **1.0:** No gain change (0 dB, default)
    /// - **2.0:** Double amplitude (+6 dB)
    /// - **4.0:** Maximum boost (+12 dB)
    ///
    /// **Formula:** Gain (dB) = 20 × log₁₀(value)
    /// **Default:** 1.0
    VoiceGain,
}

impl From<ProcessorParameter> for AicProcessorParameter::Type {
    fn from(parameter: ProcessorParameter) -> Self {
        match parameter {
            ProcessorParameter::Bypass => AIC_PROCESSOR_PARAMETER_BYPASS,
            ProcessorParameter::EnhancementLevel => AIC_PROCESSOR_PARAMETER_ENHANCEMENT_LEVEL,
            ProcessorParameter::VoiceGain => AIC_PROCESSOR_PARAMETER_VOICE_GAIN,
        }
    }
}

pub struct ProcessorContext {
    /// Raw pointer to the C processor context structure
    inner: *mut AicProcessorContext,
}

impl ProcessorContext {
    /// Creates a new Processor context.
    pub(crate) fn new(ctx_ptr: *mut AicProcessorContext) -> Self {
        Self { inner: ctx_ptr }
    }

    fn as_const_ptr(&self) -> *const AicProcessorContext {
        self.inner as *const AicProcessorContext
    }

    /// Modifies a processor parameter.
    ///
    /// All parameters can be changed during audio processing.
    /// This function can be called from any thread.
    ///
    /// # Arguments
    ///
    /// * `parameter` - Parameter to modify
    /// * `value` - New parameter value. See parameter documentation for ranges
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if the parameter cannot be set.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorParameter, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let proc_ctx = processor.processor_context();
    /// proc_ctx.set_parameter(ProcessorParameter::EnhancementLevel, 0.8).unwrap();
    /// ```
    pub fn set_parameter(&self, parameter: ProcessorParameter, value: f32) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        let error_code = unsafe {
            aic_processor_context_set_parameter(self.as_const_ptr(), parameter.into(), value)
        };
        handle_error(error_code)
    }

    /// Retrieves the current value of a parameter.
    ///
    /// This function can be called from any thread.
    ///
    /// # Arguments
    ///
    /// * `parameter` - Parameter to query
    ///
    /// # Returns
    ///
    /// Returns `Ok(value)` containing the current parameter value, or an `AicError` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, ProcessorParameter, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let processor_context = processor.processor_context();
    /// let enhancement_level = processor_context.parameter(ProcessorParameter::EnhancementLevel).unwrap();
    /// println!("Current enhancement level: {enhancement_level}");
    /// ```
    pub fn parameter(&self, parameter: ProcessorParameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        // - `value` points to stack storage for output.
        let error_code = unsafe {
            aic_processor_context_get_parameter(self.as_const_ptr(), parameter.into(), &mut value)
        };
        handle_error(error_code)?;
        Ok(value)
    }

    /// Returns the total output delay in samples for the current audio configuration.
    ///
    /// This function provides the complete end-to-end latency introduced by the model,
    /// which includes both algorithmic processing delay and any buffering overhead.
    /// Use this value to synchronize enhanced audio with other streams or to implement
    /// delay compensation in your application.
    ///
    /// **Delay behavior:**
    /// - **Before initialization:** Returns the base processing delay using the model's
    ///   optimal frame size at its native sample rate
    /// - **After initialization:** Returns the actual delay for your specific configuration,
    ///   including any additional buffering introduced by non-optimal frame sizes
    ///
    /// **Important:** The delay value is always expressed in samples at the sample rate
    /// you configured during `initialize`. To convert to time units:
    /// `delay_ms = (delay_samples * 1000) / sample_rate`
    ///
    /// **Note:** Using frame sizes different from the optimal value returned by
    /// `optimal_num_frames` will increase the delay beyond the model's base latency.
    ///
    /// # Returns
    ///
    /// Returns the delay in samples.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let processor_context = processor.processor_context();
    /// let delay = processor_context.output_delay();
    /// println!("Output delay: {} samples", delay);
    /// ```
    pub fn output_delay(&self) -> usize {
        let mut delay: usize = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        // - `delay` points to stack storage for output.
        let error_code =
            unsafe { aic_processor_context_get_output_delay(self.as_const_ptr(), &mut delay) };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_get_output_delay` is documented to always succeed if given a valid processor pointer.
        assert_success(
            error_code,
            "`aic_get_output_delay` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        delay
    }

    /// Clears all internal state and buffers.
    ///
    /// Call this when the audio stream is interrupted or when seeking
    /// to prevent artifacts from previous audio content.
    ///
    /// The processor stays initialized to the configured settings.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if the reset fails.
    ///
    /// # Thread Safety
    /// Real-time safe. Can be called from audio processing threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let processor_context = processor.processor_context();
    /// processor_context.reset().unwrap();
    /// ```
    pub fn reset(&self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        let error_code = unsafe { aic_processor_context_reset(self.as_const_ptr()) };
        handle_error(error_code)
    }
}

impl Drop for ProcessorContext {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_processor_context_destroy(self.inner) };
        }
    }
}

// Safety: The underlying C library should be thread-safe for individual ProcessorContext instances
unsafe impl Send for ProcessorContext {}
unsafe impl Sync for ProcessorContext {}

/// High-level wrapper for the ai-coustics audio enhancement model.
///
/// This struct provides a safe, Rust-friendly interface to the underlying C library.
/// It handles memory management automatically and converts C-style error codes
/// to Rust `Result` types.
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Model, ProcessorConfig, Processor};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
/// let config = ProcessorConfig {
///     sample_rate: model.optimal_sample_rate(),
///     num_channels: 2,
///     num_frames: 1024,
///     allow_variable_frames: false,
/// };
///
/// let mut processor = Processor::new(&model, &license_key).unwrap();
/// processor.initialize(config).unwrap();
///
/// let mut audio_buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
/// processor.process_interleaved(&mut audio_buffer).unwrap();
/// ```
pub struct Processor<'a> {
    /// Raw pointer to the C processor structure
    inner: *mut AicProcessor,
    /// Configured number of channels
    num_channels: Option<u16>,
    /// Marker to tie the lifetime of the processor to the lifetime of the model's weights
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Processor<'a> {
    /// Creates a new audio enhancement model instance.
    ///
    /// Multiple models can be created to process different audio streams simultaneously
    /// or to switch between different enhancement algorithms during runtime.
    ///
    /// # Arguments
    ///
    /// * `model` - The loaded model instance
    /// * `license_key` - license key for the ai-coustics SDK
    ///   (generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com/))
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Processor` instance or an `AicError` if creation fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let processor = Processor::new(&model, &license_key).unwrap();
    /// ```
    pub fn new(model: &Model<'a>, license_key: &str) -> Result<Self, AicError> {
        SET_WRAPPER_ID.call_once(|| unsafe {
            // SAFETY:
            // - This function has no safety requirements, it's unsafe because it's FFI.
            aic_set_sdk_wrapper_id(2);
        });

        let mut processor_ptr: *mut AicProcessor = ptr::null_mut();
        let c_license_key =
            CString::new(license_key).map_err(|_| AicError::LicenseFormatInvalid)?;

        // SAFETY:
        // - `processor_ptr` and `model` pointers are valid for the duration of the call.
        // - `model` is kept alive internally as it stores the memory in an `Arc`
        // - `c_license_key` is a NUL-terminated CString.
        let error_code = unsafe {
            aic_processor_create(
                &mut processor_ptr,
                model.as_const_ptr(),
                c_license_key.as_ptr(),
            )
        };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !processor_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: processor_ptr,
            num_channels: None,
            marker: PhantomData,
        })
    }

    /// Initializes the processor with the given configuration.
    ///
    /// This is a convenience method that calls [`Processor::initialize`] internally and returns `self`.
    /// The processor is immediately ready to process audio after calling this method, so you don't
    /// need to call [`Processor::initialize`] separately.
    ///
    /// # Arguments
    ///
    /// * `config` - Audio processing configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` with the initialized processor, or an `AicError` if initialization fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor, ProcessorConfig};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/model.aicmodel")?;
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    ///
    /// let mut processor = Processor::new(&model, &license_key)?.with_config(config)?;
    ///
    /// // Processor is ready to use - no need to call initialize()
    /// let mut audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    /// processor.process_interleaved(&mut audio).unwrap();
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn with_config(mut self, config: ProcessorConfig) -> Result<Self, AicError> {
        self.initialize(config)?;
        Ok(self)
    }

    /// Creates a [ProcessorContext](crate::processor::ProcessorContext) instance.
    /// This can be used to control all parameters and other settings of the processor.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let processor = Processor::new(&model, &license_key).unwrap();
    /// let processor_context = processor.processor_context();
    /// ```
    pub fn processor_context(&self) -> ProcessorContext {
        let mut processor_context: *mut AicProcessorContext = ptr::null_mut();

        // SAFETY:
        // - `processor_context` is valid output storage.
        // - `self.as_const_ptr()` is a live processor pointer.
        let error_code =
            unsafe { aic_processor_context_create(&mut processor_context, self.as_const_ptr()) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !processor_context.is_null(),
            "C library returned success but null pointer"
        );

        ProcessorContext::new(processor_context)
    }

    /// Creates a [Voice Activity Detector Context](crate::vad::VadContext) instance.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let processor = Processor::new(&model, &license_key).unwrap();
    /// let vad = processor.vad_context();
    /// ```
    pub fn vad_context(&self) -> crate::VadContext {
        let mut vad_ptr: *mut AicVadContext = ptr::null_mut();

        // SAFETY:
        // - `vad_ptr` is valid output storage.
        // - `self.as_const_ptr()` is a live processor pointer.
        let error_code = unsafe { aic_vad_context_create(&mut vad_ptr, self.as_const_ptr()) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !vad_ptr.is_null(),
            "C library returned success but null pointer"
        );

        crate::vad::VadContext::new(vad_ptr)
    }

    /// Configures the model for specific audio settings.
    ///
    /// This function must be called before processing any audio.
    /// For the lowest delay use the sample rate and frame size returned by
    /// [`Model::optimal_sample_rate`] and [`Model::optimal_num_frames`].
    ///
    /// # Arguments
    ///
    /// * `config` - Audio processing configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if initialization fails.
    ///
    /// # Warning
    /// Do not call from audio processing threads as this allocates memory.
    ///
    /// # Note
    /// All channels are mixed to mono for processing. To process channels
    /// independently, create separate [`Processor`] instances.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = ProcessorConfig::optimal(&model);
    /// processor.initialize(config).unwrap();
    /// ```
    pub fn initialize(&mut self, config: ProcessorConfig) -> Result<(), AicError> {
        // SAFETY:
        // - `self.inner` is a valid pointer to a live processor.
        let error_code = unsafe {
            aic_processor_initialize(
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

    /// Processes audio with separate buffers for each channel (planar layout).
    ///
    /// Enhances speech in the provided audio buffers in-place.
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
    /// * `audio` - Array of mutable channel buffer slices to be enhanced in-place.
    ///             Each channel buffer must be exactly of size `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value.
    ///
    /// # Note
    ///
    /// Maximum supported number of channels is 16. Exceeding this will return an error.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    /// processor.initialize(config).unwrap();
    /// let mut audio = vec![vec![0.0f32; config.num_frames]; config.num_channels as usize];
    /// processor.process_planar(&mut audio).unwrap();
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn process_planar<V: AsMut<[f32]>>(&mut self, audio: &mut [V]) -> Result<(), AicError> {
        const MAX_CHANNELS: u16 = 16;

        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ModelNotInitialized);
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
            audio[0].as_mut().len()
        };

        let mut audio_ptrs = [std::ptr::null_mut::<f32>(); MAX_CHANNELS as usize];
        for (i, channel) in audio.iter_mut().enumerate() {
            // Check that all channels have the same number of frames
            if channel.as_mut().len() != num_frames {
                return Err(AicError::AudioConfigMismatch);
            }
            audio_ptrs[i] = channel.as_mut().as_mut_ptr();
        }

        // SAFETY:
        // - `self.inner` is a valid pointer to a live processor.
        // - `audio_ptrs` holds valid, writable channel pointers containing `num_frames` samples each.
        let error_code = unsafe {
            aic_processor_process_planar(self.inner, audio_ptrs.as_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }

    /// Processes audio with interleaved channel data.
    ///
    /// Enhances speech in the provided audio buffer in-place.
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
    /// * `audio` - Interleaved audio buffer to be enhanced in-place.
    ///             Must be exactly of size `num_channels` * `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value per channel.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    /// processor.initialize(config).unwrap();
    /// let mut audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    /// processor.process_interleaved(&mut audio).unwrap();
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn process_interleaved(&mut self, audio: &mut [f32]) -> Result<(), AicError> {
        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ModelNotInitialized);
        };

        if !audio.len().is_multiple_of(num_channels as usize) {
            return Err(AicError::AudioConfigMismatch);
        }

        let num_frames = audio.len() / num_channels as usize;

        // SAFETY:
        // - `self.inner` is a valid pointer to a live processor.
        // - `audio` points to a contiguous f32 slice of correct length.
        let error_code = unsafe {
            aic_processor_process_interleaved(
                self.inner,
                audio.as_mut_ptr(),
                num_channels,
                num_frames,
            )
        };

        handle_error(error_code)
    }

    /// Processes audio with sequential channel data.
    ///
    /// Enhances speech in the provided audio buffer in-place.
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
    /// * `audio` - Sequential audio buffer to be enhanced in-place.
    ///             Must be exactly of size `num_channels` * `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value per channel.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor, ProcessorConfig};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = ProcessorConfig::optimal(&model).with_num_channels(2);;
    /// processor.initialize(config).unwrap();
    /// let mut audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    /// processor.process_sequential(&mut audio).unwrap();
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn process_sequential(&mut self, audio: &mut [f32]) -> Result<(), AicError> {
        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ModelNotInitialized);
        };

        if !audio.len().is_multiple_of(num_channels as usize) {
            return Err(AicError::AudioConfigMismatch);
        }

        let num_frames = audio.len() / num_channels as usize;

        // SAFETY: `self.inner` is initialized; `audio` length has been validated.
        let error_code = unsafe {
            aic_processor_process_sequential(
                self.inner,
                audio.as_mut_ptr(),
                num_channels,
                num_frames,
            )
        };

        handle_error(error_code)
    }

    fn as_const_ptr(&self) -> *const AicProcessor {
        self.inner as *const AicProcessor
    }
}

impl<'a> Drop for Processor<'a> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_processor_destroy(self.inner) };
        }
    }
}

// SAFETY: Everything in Processor is Send, with the exception of the inner raw pointer.
// The Processor only uses the raw pointer according to the safety contracts of the
// unsafe APIs that require the pointer, and the Processor does not expose access to the
// raw pointer in any of its methods. Therefore, it safe to implement Send for Processor.
unsafe impl<'a> Send for Processor<'a> {}

// SAFETY: Processor does not expose any interior mutability, and all unsafe APIs that make use of
// the inner raw pointer are only used in methods that take &mut self, which upholds the thread safety
// contracts required by the unsafe APIs. Therefore, it is safe to implement Sync for Processor.
unsafe impl<'a> Sync for Processor<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    fn find_existing_model(target_dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(target_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.contains("quail_xxs_48khz") && name.ends_with(".aicmodel"))
                .unwrap_or(false)
            {
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Downloads the default test model `quail-xxs-48khz` into the crate's `target/` directory.
    /// Returns the path to the downloaded model file.
    fn get_quail_xxs_48khz() -> Result<PathBuf, AicError> {
        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");

        if let Some(existing) = find_existing_model(&target_dir) {
            return Ok(existing);
        }

        #[cfg(feature = "download-model")]
        {
            return Model::download("quail-xxs-48khz", target_dir);
        }

        #[cfg(not(feature = "download-model"))]
        {
            panic!(
                "Model `quail-xxs-48khz` not found in {} and `download-model` feature is disabled",
                target_dir.display()
            );
        }
    }

    fn load_test_model() -> Result<(Model<'static>, String), AicError> {
        let license_key = std::env::var("AIC_SDK_LICENSE")
            .expect("AIC_SDK_LICENSE environment variable must be set for tests");

        let model_path = get_quail_xxs_48khz()?;
        let model = Model::from_file(&model_path)?;

        Ok((model, license_key))
    }

    #[test]
    fn model_creation_and_basic_operations() {
        dbg!(crate::get_sdk_version());
        dbg!(crate::get_compatible_model_version());

        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![vec![0.0f32; config.num_frames]; num_channels];
        let mut audio_refs: Vec<&mut [f32]> =
            audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();

        processor.process_planar(&mut audio_refs).unwrap();
    }

    #[test]
    fn process_interleaved_fixed_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.process_interleaved(&mut audio).unwrap();
    }

    #[test]
    fn process_planar_fixed_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.process_planar(&mut audio).unwrap();
    }

    #[test]
    fn process_sequential_fixed_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.process_sequential(&mut audio).unwrap();
    }

    #[test]
    fn process_interleaved_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model)
            .with_num_channels(2)
            .with_allow_variable_frames(true);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.process_interleaved(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        processor.process_interleaved(&mut audio).unwrap();
    }

    #[test]
    fn process_planar_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model)
            .with_num_channels(2)
            .with_allow_variable_frames(true);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.process_planar(&mut audio).unwrap();

        let mut left = vec![0.0f32; 20];
        let mut right = vec![0.0f32; 20];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.process_planar(&mut audio).unwrap();
    }

    #[test]
    fn process_sequential_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model)
            .with_num_channels(2)
            .with_allow_variable_frames(true);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.process_sequential(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        processor.process_sequential(&mut audio).unwrap();
    }

    #[test]
    fn process_interleaved_variable_frames_fails_without_allow_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.process_interleaved(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        let result = processor.process_interleaved(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }

    #[test]
    fn process_planar_variable_frames_fails_without_allow_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.process_planar(&mut audio).unwrap();

        let mut left = vec![0.0f32; 20];
        let mut right = vec![0.0f32; 20];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        let result = processor.process_planar(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }

    #[test]
    fn process_sequential_variable_frames_fails_without_allow_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();

        let num_channels = config.num_channels as usize;
        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.process_sequential(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        let result = processor.process_sequential(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }

    #[test]
    fn model_can_be_dropped() {
        let (model, license_key) = load_test_model().unwrap();
        let config = ProcessorConfig::optimal(&model).with_num_channels(2);

        let mut processor = Processor::new(&model, &license_key)
            .unwrap()
            .with_config(config)
            .unwrap();
        drop(model);

        let num_channels = config.num_channels as usize;
        let mut audio = vec![vec![0.0f32; config.num_frames]; num_channels];
        let mut audio_refs: Vec<&mut [f32]> =
            audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();

        processor.process_planar(&mut audio_refs).unwrap();
    }

    #[test]
    fn processor_is_send_and_sync() {
        // Compile-time check that Processor implements Send and Sync.
        // This ensures the processor can be safely moved to another thread.
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Send>() {}

        assert_send::<Processor>();
        assert_sync::<Processor>();
    }
}

#[doc(hidden)]
mod _compile_fail_tests {
    //! Compile-fail regression: a `Processor`'s model buffer must not be dropped before the processor.
    //!
    //! ```rust,compile_fail
    //! use aic_sdk::{Model, Processor, ProcessorConfig};
    //!
    //! fn main() {
    //!     let buffer = vec![0u8; 64];
    //!     let model = Model::from_buffer(&buffer).unwrap();
    //!     let config = ProcessorConfig::optimal(&model).with_num_channels(2);
    //!
    //!     let mut processor = Processor::new(&model, "license")
    //!         .unwrap()
    //!         .with_config(config)
    //!         .unwrap();
    //!
    //!     drop(model); // Model can be dropped without issues
    //!
    //!     drop(buffer); // This should fail to compile
    //!
    //!     let num_channels = config.num_channels as usize;
    //!     let mut audio = vec![vec![0.0f32; config.num_frames]; num_channels];
    //!     processor.process_planar(&mut audio).unwrap();
    //! }
    //! ```
}
