use crate::{error::*, model::Model};

use aic_sdk_sys::{AicParameter::*, *};

use std::{ffi::CString, marker::PhantomData, ptr, sync::Once};

static SET_WRAPPER_ID: Once = Once::new();

/// Audio processing configuration passed to [`Processor::initialize`].
///
/// Use [`Processor::optimal_config`] as a starting point, then adjust fields
/// to match your stream layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Config {
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

/// Configurable parameters for audio enhancement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Parameter {
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

impl From<Parameter> for AicParameter::Type {
    fn from(parameter: Parameter) -> Self {
        match parameter {
            Parameter::Bypass => AIC_PARAMETER_BYPASS,
            Parameter::EnhancementLevel => AIC_PARAMETER_ENHANCEMENT_LEVEL,
            Parameter::VoiceGain => AIC_PARAMETER_VOICE_GAIN,
        }
    }
}

/// High-level wrapper for the ai-coustics audio enhancement model.
///
/// This struct provides a safe, Rust-friendly interface to the underlying C library.
/// It handles memory management automatically and converts C-style error codes
/// to Rust `Result` types.
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Config, Model, Processor};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
/// let mut processor = Processor::new(&model, &license_key).unwrap();
///
/// let config = Config {
///     num_channels: 2,
///     num_frames: 1024,
///     ..processor.optimal_config()
/// };
/// processor.initialize(&config).unwrap();
///
/// let mut audio_buffer = vec![0.0f32; config.num_channels * config.num_frames];
/// processor.process_interleaved(&mut audio_buffer).unwrap();
/// ```
pub struct Processor<'a, 'm> {
    /// Raw pointer to the C processor structure
    inner: *mut AicProcessor,
    /// Configured number of channels
    num_channels: Option<u16>,
    /// Phantom data to tie the lifetime to the Model
    marker: PhantomData<&'a Model<'m>>,
}

impl<'a, 'm> Processor<'a, 'm> {
    /// Creates a new audio enhancement model instance.
    ///
    /// Multiple models can be created to process different audio streams simultaneously
    /// or to switch between different enhancement algorithms during runtime.
    ///
    /// # Arguments
    ///
    /// * `model` - The loaded model instance
    /// * `license_key` - Valid license key for the AIC SDK
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Model` instance or an `AicError` if creation fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let processor = Processor::new(&model, &license_key).unwrap();
    /// ```
    pub fn new(model: &'a Model<'m>, license_key: &str) -> Result<Self, AicError> {
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

    /// Creates a [Voice Activity Detector](crate::vad::Vad) instance.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// let processor = Processor::new(&model, &license_key).unwrap();
    /// let vad = processor.create_vad();
    /// ```
    pub fn create_vad(&self) -> crate::Vad {
        let mut vad_ptr: *mut AicVad = ptr::null_mut();

        // SAFETY:
        // - `vad_ptr` is valid output storage.
        // - `self.as_const_ptr()` is a live processor pointer.
        let error_code = unsafe { aic_vad_create(&mut vad_ptr, self.as_const_ptr()) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !vad_ptr.is_null(),
            "C library returned success but null pointer"
        );

        crate::vad::Vad::new(vad_ptr)
    }

    /// Configures the model for a specific audio format.
    ///
    /// This function must be called before processing any audio.
    /// For the lowest delay use the sample rate and frame size returned by
    /// [`Processor::optimal_sample_rate`] and [`Processor::optimal_num_frames`].
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
    /// independently, create separate model instances.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = processor.optimal_config();
    /// processor.initialize(&config).unwrap();
    /// ```
    pub fn initialize(&mut self, config: &Config) -> Result<(), AicError> {
        let num_channels_u16: u16 = config
            .num_channels
            .try_into()
            .map_err(|_| AicError::AudioConfigUnsupported)?;

        // SAFETY:
        // - `self.inner` is a valid pointer to a live processor.
        let error_code = unsafe {
            aic_processor_initialize(
                self.inner,
                config.sample_rate,
                num_channels_u16,
                config.num_frames,
                config.allow_variable_frames,
            )
        };

        handle_error(error_code)?;
        self.num_channels = Some(config.num_channels);
        Ok(())
    }

    /// Returns a [`Config`] pre-filled with the model's optimal sample rate and frame size.
    ///
    /// This helper uses [`Processor::optimal_sample_rate`] and
    /// [`Processor::optimal_num_frames`] to minimize latency, and defaults to
    /// mono processing with fixed frame counts. Adjust `num_channels` or
    /// `allow_variable_frames` as needed before calling [`Processor::initialize`].
    pub fn optimal_config(&self) -> Config {
        let sample_rate = self.optimal_sample_rate();
        let num_frames = self.optimal_num_frames(sample_rate);
        Config {
            sample_rate,
            num_channels: 1,
            num_frames,
            allow_variable_frames: false,
        }
    }

    /// Clears all internal state and buffers.
    ///
    /// Call this when the audio stream is interrupted or when seeking
    /// to prevent artifacts from previous audio content.
    ///
    /// The model stays initialized to the configured settings.
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
    /// processor.reset().unwrap();
    /// ```
    pub fn reset(&self) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        let error_code = unsafe { aic_processor_reset(self.as_const_ptr()) };
        handle_error(error_code)
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
    /// # use aic_sdk::{Config, Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = Config { num_channels: 2, ..processor.optimal_config() };
    /// let mut audio = vec![vec![0.0f32; config.num_frames]; config.num_channels];
    /// let mut audio_refs: Vec<&mut [f32]> = audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
    /// processor.initialize(&config).unwrap();
    /// processor.process_planar(&mut audio_refs).unwrap();
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn process_planar(&mut self, audio: &mut [&mut [f32]]) -> Result<(), AicError> {
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

        let num_frames = if audio.is_empty() { 0 } else { audio[0].len() };
        let num_channels = num_channels as u16;

        let mut audio_ptrs = [std::ptr::null_mut::<f32>(); MAX_CHANNELS as usize];
        for (i, channel) in audio.iter_mut().enumerate() {
            // Check that all channels have the same number of frames
            if channel.len() != num_frames {
                return Err(AicError::AudioConfigMismatch);
            }
            audio_ptrs[i] = channel.as_mut_ptr();
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
    /// # use aic_sdk::{Config, Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = Config { num_channels: 2, ..processor.optimal_config() };
    /// let mut audio = vec![0.0f32; config.num_channels * config.num_frames];
    /// processor.initialize(&config).unwrap();
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
        let num_channels = num_channels as u16;

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
    /// # use aic_sdk::{Config, Model, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let mut processor = Processor::new(&model, &license_key).unwrap();
    /// let config = Config { num_channels: 2, ..processor.optimal_config() };
    /// let mut audio = vec![0.0f32; config.num_channels * config.num_frames];
    /// processor.initialize(&config).unwrap();
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
        let num_channels = num_channels as u16;

        // SAFETY: `self.inner` is initialized, `audio` points to a contiguous f32 slice of correct length.
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

    /// Modifies a model parameter.
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
    /// # use aic_sdk::{Model, Parameter, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// processor.set_parameter(Parameter::EnhancementLevel, 0.8).unwrap();
    /// ```
    pub fn set_parameter(&self, parameter: Parameter, value: f32) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        let error_code =
            unsafe { aic_processor_set_parameter(self.as_const_ptr(), parameter.into(), value) };
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
    /// # use aic_sdk::{Model, Parameter, Processor};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// let enhancement_level = processor.parameter(Parameter::EnhancementLevel).unwrap();
    /// println!("Current enhancement level: {enhancement_level}");
    /// ```
    pub fn parameter(&self, parameter: Parameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        // - `value` points to stack storage for output.
        let error_code = unsafe {
            aic_processor_get_parameter(self.as_const_ptr(), parameter.into(), &mut value)
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
    /// let delay = processor.output_delay();
    /// println!("Output delay: {} samples", delay);
    /// ```
    pub fn output_delay(&self) -> usize {
        let mut delay: usize = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        // - `delay` points to stack storage for output.
        let error_code = unsafe { aic_get_output_delay(self.as_const_ptr(), &mut delay) };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_get_output_delay` is documented to always succeed if given a valid processor pointer.
        assert_success(
            error_code,
            "`aic_get_output_delay` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        delay
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
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// let optimal_rate = processor.optimal_sample_rate();
    /// println!("Optimal sample rate: {optimal_rate} Hz");
    /// ```
    pub fn optimal_sample_rate(&self) -> u32 {
        let mut sample_rate: u32 = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        // - `sample_rate` points to stack storage for output.
        let error_code =
            unsafe { aic_get_optimal_sample_rate(self.as_const_ptr(), &mut sample_rate) };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_get_optimal_sample_rate` is documented to always succeed if given a valid processor pointer.
        assert_success(
            error_code,
            "`aic_get_optimal_sample_rate` failed. This is a bug, please open an issue on GitHub for further investigation.",
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
    /// [`Processor::initialize`] to determine the best frame count for minimal latency.
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
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let sample_rate = processor.optimal_sample_rate();
    /// let optimal_frames = processor.optimal_num_frames(sample_rate);
    /// println!("Optimal frame count: {optimal_frames}");
    /// ```
    pub fn optimal_num_frames(&self, sample_rate: u32) -> usize {
        let mut num_frames: usize = 0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live processor.
        // - `num_frames` points to stack storage for output.
        let error_code = unsafe {
            aic_get_optimal_num_frames(self.as_const_ptr(), sample_rate, &mut num_frames)
        };

        // This should never fail. If it does, it's a bug in the SDK.
        // `aic_get_optimal_num_frames` is documented to always succeed if given valid pointers.
        assert_success(
            error_code,
            "`aic_get_optimal_num_frames` failed. This is a bug, please open an issue on GitHub for further investigation.",
        );

        num_frames
    }

    fn as_const_ptr(&self) -> *const AicProcessor {
        self.inner as *const AicProcessor
    }
}

impl<'a, 'm> Drop for Processor<'a, 'm> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_processor_destroy(self.inner) };
        }
    }
}

// SAFETY:
// - The Processor struct safely wraps the AicProcessor object and uses the C library's APIs
//   according to the documented thread-safety guarantees.
unsafe impl<'a, 'm> Send for Processor<'a, 'm> {}
unsafe impl<'a, 'm> Sync for Processor<'a, 'm> {}

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
        dbg!(crate::get_version());
        dbg!(crate::get_compatible_model_version());

        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![vec![0.0f32; config.num_frames]; num_channels];
        let mut audio_refs: Vec<&mut [f32]> =
            audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();

        processor.initialize(&config).unwrap();
        processor.process_planar(&mut audio_refs).unwrap();
    }

    #[test]
    fn process_interleaved_fixed_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.initialize(&config).unwrap();
        processor.process_interleaved(&mut audio).unwrap();
    }

    #[test]
    fn process_planar_fixed_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];

        processor.initialize(&config).unwrap();
        processor.process_planar(&mut audio).unwrap();
    }

    #[test]
    fn process_sequential_fixed_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.initialize(&config).unwrap();
        processor.process_sequential(&mut audio).unwrap();
    }

    #[test]
    fn process_interleaved_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            allow_variable_frames: true,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.initialize(&config).unwrap();
        processor.process_interleaved(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        processor.process_interleaved(&mut audio).unwrap();
    }

    #[test]
    fn process_planar_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            allow_variable_frames: true,
            ..processor.optimal_config()
        };

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.initialize(&config).unwrap();
        processor.process_planar(&mut audio).unwrap();

        let mut left = vec![0.0f32; 20];
        let mut right = vec![0.0f32; 20];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.process_planar(&mut audio).unwrap();
    }

    #[test]
    fn process_sequential_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            allow_variable_frames: true,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.initialize(&config).unwrap();
        processor.process_sequential(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        processor.process_sequential(&mut audio).unwrap();
    }

    #[test]
    fn process_interleaved_variable_frames_fails_without_allow_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.initialize(&config).unwrap();
        processor.process_interleaved(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        let result = processor.process_interleaved(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }

    #[test]
    fn process_planar_variable_frames_fails_without_allow_variable_frames() {
        let (model, license_key) = load_test_model().unwrap();
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let mut left = vec![0.0f32; config.num_frames];
        let mut right = vec![0.0f32; config.num_frames];
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()];
        processor.initialize(&config).unwrap();
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
        let mut processor = Processor::new(&model, &license_key).unwrap();

        let config = Config {
            num_channels: 2,
            ..processor.optimal_config()
        };

        let num_channels = config.num_channels as usize;

        let mut audio = vec![0.0f32; num_channels * config.num_frames];
        processor.initialize(&config).unwrap();
        processor.process_sequential(&mut audio).unwrap();

        let mut audio = vec![0.0f32; num_channels * 20];
        let result = processor.process_sequential(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }
}

#[doc(hidden)]
mod _compile_fail_tests {
    //! Compile-fail regression: a `Processor` must not outlive its `Model`.
    //! This snippet should fail to compile and ensures we keep that guarantee.
    //!
    //! ```rust,compile_fail
    //! use aic_sdk::{Model, Processor};
    //!
    //! fn leak_processor<'a>() -> Processor<'a, 'a> {
    //!     let license_key = "dummy-license";
    //!     let processor = {
    //!         let bytes = vec![0u8; 64];
    //!         let model = Model::from_buffer(&bytes).unwrap();
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
