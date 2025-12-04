use crate::error::*;

use aic_sdk_sys::{AicEnhancementParameter::*, AicModelType::*, *};

use std::{ffi::CString, ptr, sync::Once};

static SET_WRAPPER_ID: Once = Once::new();

/// Available model types for audio enhancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelType {
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 48 kHz
    /// - Native num frames: 480
    /// - Processing latency: 30 ms
    QuailL48,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 16 kHz
    /// - Native num frames: 160
    /// - Processing latency: 30 ms
    QuailL16,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 8 kHz
    /// - Native num frames: 80
    /// - Processing latency: 30 ms
    QuailL8,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 48 kHz
    /// - Native num frames: 480
    /// - Processing latency: 30 ms
    QuailS48,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 16 kHz
    /// - Native num frames: 160
    /// - Processing latency: 30 ms
    QuailS16,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 8 kHz
    /// - Native num frames: 80
    /// - Processing latency: 30 ms
    QuailS8,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 48 kHz
    /// - Native num frames: 480
    /// - Processing latency: 10 ms
    QuailXS,
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 48 kHz
    /// - Native num frames: 480
    /// - Processing latency: 10 ms
    QuailXXS,
    /// Special model optimized for human-to-machine interaction (e.g., voice agents, speech-to-text)
    /// that uses fixed enhancement parameters that cannot be changed during runtime.
    /// **Specifications:**
    /// - Window length: 10 ms
    /// - Native sample rate: 16 kHz
    /// - Native num frames: 160
    /// - Processing latency: 30 ms
    QuailSTT,
}

impl From<ModelType> for AicModelType::Type {
    fn from(model_type: ModelType) -> Self {
        match model_type {
            ModelType::QuailL48 => AIC_MODEL_TYPE_QUAIL_L48,
            ModelType::QuailL16 => AIC_MODEL_TYPE_QUAIL_L16,
            ModelType::QuailL8 => AIC_MODEL_TYPE_QUAIL_L8,
            ModelType::QuailS48 => AIC_MODEL_TYPE_QUAIL_S48,
            ModelType::QuailS16 => AIC_MODEL_TYPE_QUAIL_S16,
            ModelType::QuailS8 => AIC_MODEL_TYPE_QUAIL_S8,
            ModelType::QuailXS => AIC_MODEL_TYPE_QUAIL_XS,
            ModelType::QuailXXS => AIC_MODEL_TYPE_QUAIL_XXS,
            ModelType::QuailSTT => AIC_MODEL_TYPE_QUAIL_STT,
        }
    }
}

/// Configurable parameters for audio enhancement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnhancementParameter {
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

impl From<EnhancementParameter> for AicEnhancementParameter::Type {
    fn from(parameter: EnhancementParameter) -> Self {
        match parameter {
            EnhancementParameter::Bypass => AIC_ENHANCEMENT_PARAMETER_BYPASS,
            EnhancementParameter::EnhancementLevel => AIC_ENHANCEMENT_PARAMETER_ENHANCEMENT_LEVEL,
            EnhancementParameter::VoiceGain => AIC_ENHANCEMENT_PARAMETER_VOICE_GAIN,
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
    /// Configured number of channels
    num_channels: Option<u16>,
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
    pub fn new(model_type: ModelType, license_key: &str) -> Result<Self, AicError> {
        SET_WRAPPER_ID.call_once(|| unsafe {
            aic_set_sdk_wrapper_id(2);
        });

        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_license_key =
            CString::new(license_key).map_err(|_| AicError::LicenseFormatInvalid)?;

        let error_code =
            unsafe { aic_model_create(&mut model_ptr, model_type.into(), c_license_key.as_ptr()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: model_ptr,
            num_channels: None,
        })
    }

    /// Creates a [Voice Activity Detector](crate::vad::Vad) instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// let vad = model.create_vad();
    /// ```
    pub fn create_vad(&self) -> crate::Vad {
        let mut vad_ptr: *mut AicVad = ptr::null_mut();

        let error_code = unsafe { aic_vad_create(&mut vad_ptr, self.inner) };

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
    /// `optimal_sample_rate` and `optimal_num_frames`.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Audio sample rate in Hz (8000 - 192000)
    /// * `num_channels` - Number of audio channels (1 for mono, 2 for stereo, etc.)
    /// * `num_frames` - Number of samples per channel in each process call
    /// * `allow_variable_frames` - Allows varying frame counts per process call (up to `num_frames`), but increases delay.
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
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// model.initialize(48000, 1, 1024, true).unwrap();
    /// ```
    pub fn initialize(
        &mut self,
        sample_rate: u32,
        num_channels: u16,
        num_frames: usize,
        allow_variable_frames: bool,
    ) -> Result<(), AicError> {
        let error_code = unsafe {
            aic_model_initialize(
                self.inner,
                sample_rate,
                num_channels,
                num_frames,
                allow_variable_frames,
            )
        };

        handle_error(error_code)?;
        self.num_channels = Some(num_channels);
        Ok(())
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
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// model.reset().unwrap();
    /// ```
    pub fn reset(&mut self) -> Result<(), AicError> {
        let error_code = unsafe { aic_model_reset(self.inner) };
        handle_error(error_code)
    }

    /// Processes audio with separate buffers for each channel (planar layout).
    ///
    /// Enhances speech in the provided audio buffers in-place.
    ///
    /// The planar function allows a maximum of 16 channels.
    ///
    /// # Arguments
    ///
    /// * `audio` - Array of channel buffer pointers to be enhanced in-place.
    ///             Each channel buffer must be exactly of size `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// let mut audio = vec![vec![0.0f32; 480]; 2]; // 2 channels, 480 frames each
    /// let mut audio_refs: Vec<&mut [f32]> = audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
    /// model.initialize(48000, 2, 480, false).unwrap();
    /// model.process_planar(&mut audio_refs).unwrap();
    /// ```
    #[allow(clippy::doc_overindented_list_items)]
    pub fn process_planar(&mut self, audio: &mut [&mut [f32]]) -> Result<(), AicError> {
        const MAX_CHANNELS: usize = 16;

        let Some(num_channels) = self.num_channels else {
            return Err(AicError::ModelNotInitialized);
        };

        if audio.len() != num_channels as usize {
            return Err(AicError::AudioConfigMismatch);
        }

        let num_frames = if audio.is_empty() { 0 } else { audio[0].len() };

        let mut audio_ptrs = [std::ptr::null_mut::<f32>(); MAX_CHANNELS];
        for (i, channel) in audio.iter_mut().enumerate() {
            // Check that all channels have the same number of frames
            if channel.len() != num_frames {
                return Err(AicError::AudioConfigMismatch);
            }
            audio_ptrs[i] = channel.as_mut_ptr();
        }

        let error_code = unsafe {
            aic_model_process_planar(self.inner, audio_ptrs.as_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }

    /// Processes audio with interleaved channel data.
    ///
    /// Enhances speech in the provided audio buffer in-place.
    ///
    /// # Arguments
    ///
    /// * `audio` - Interleaved audio buffer to be enhanced in-place.
    ///             Must be exactly of size `num_channels` * `num_frames`,
    ///             or if `allow_variable_frames` was enabled, less than the initialization value per channel).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `AicError` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// let mut audio = vec![0.0f32; 2 * 480]; // 2 channels, 480 frames
    /// model.initialize(48000, 2, 480, false).unwrap();
    /// model.process_interleaved(&mut audio).unwrap();
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

        let error_code = unsafe {
            aic_model_process_interleaved(self.inner, audio.as_mut_ptr(), num_channels, num_frames)
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
    /// ```rust
    /// # use aic_sdk::{Model, ModelType, EnhancementParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// model.set_parameter(EnhancementParameter::EnhancementLevel, 0.8).unwrap();
    /// ```
    pub fn set_parameter(
        &mut self,
        parameter: EnhancementParameter,
        value: f32,
    ) -> Result<(), AicError> {
        let error_code = unsafe { aic_model_set_parameter(self.inner, parameter.into(), value) };
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
    /// ```rust
    /// # use aic_sdk::{Model, ModelType, EnhancementParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// let enhancement_level = model.parameter(EnhancementParameter::EnhancementLevel).unwrap();
    /// println!("Current enhancement level: {enhancement_level}");
    /// ```
    pub fn parameter(&self, parameter: EnhancementParameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        let error_code =
            unsafe { aic_model_get_parameter(self.inner, parameter.into(), &mut value) };
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
    /// Returns `Ok(delay_samples)` or an `AicError` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// let delay = model.output_delay().unwrap();
    /// println!("Output delay: {} samples", delay);
    /// ```
    pub fn output_delay(&self) -> Result<usize, AicError> {
        let mut delay: usize = 0;
        let error_code = unsafe { aic_get_output_delay(self.inner, &mut delay) };
        handle_error(error_code)?;
        Ok(delay)
    }

    /// Retrieves the native sample rate of the selected model.
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
    /// Returns `Ok(sample_rate)` or an `AicError` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// let optimal_rate = model.optimal_sample_rate().unwrap();
    /// println!("Optimal sample rate: {optimal_rate} Hz");
    /// ```
    pub fn optimal_sample_rate(&self) -> Result<u32, AicError> {
        let mut sample_rate: u32 = 0;
        let error_code = unsafe { aic_get_optimal_sample_rate(self.inner, &mut sample_rate) };
        handle_error(error_code)?;
        Ok(sample_rate)
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
    /// Call this function with your intended sample rate before calling `aic_model_initialize`
    /// to determine the best frame count for minimal latency.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The sample rate in Hz for which to calculate the optimal frame count.
    ///
    /// # Returns
    ///
    /// Returns `Ok(num_frames)` or an `AicError` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// # let sample_rate = model.optimal_sample_rate().unwrap();
    /// let optimal_frames = model.optimal_num_frames(sample_rate).unwrap();
    /// println!("Optimal frame count: {optimal_frames}");
    /// ```
    pub fn optimal_num_frames(&self, sample_rate: u32) -> Result<usize, AicError> {
        let mut num_frames: usize = 0;
        let error_code =
            unsafe { aic_get_optimal_num_frames(self.inner, sample_rate, &mut num_frames) };
        handle_error(error_code)?;
        Ok(num_frames)
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

// Safety: The underlying C library should be thread-safe for individual model instances
unsafe impl Send for Model {}
unsafe impl Sync for Model {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_creation_and_basic_operations() -> Result<(), AicError> {
        dbg!(crate::get_version());

        // Read license key from environment variable
        let license_key = std::env::var("AIC_SDK_LICENSE")
            .expect("AIC_SDK_LICENSE environment variable must be set for tests");

        // Test model creation with QuailL48 at optimal settings
        let mut model = Model::new(ModelType::QuailL48, &license_key)?;

        // Test initialization with QuailL48 optimal settings (48000 Hz, 480 frames)
        model.initialize(48000, 2, 480, false)?;

        let mut audio = vec![vec![0.0f32; 480]; 2]; // 2 channels, 480 frames each
        let mut audio_refs: Vec<&mut [f32]> =
            audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();

        model.process_planar(&mut audio_refs).unwrap();

        Ok(())
    }

    #[test]
    fn process_interleaved_fixed_frames() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
        let mut audio = vec![0.0f32; 2 * 480]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480, false).unwrap();
        model.process_interleaved(&mut audio).unwrap();
    }

    #[test]
    fn process_planar_fixed_frames() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
        let mut left = vec![0.0f32; 480]; // 480 frames
        let mut right = vec![0.0f32; 480]; // 480 frames
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480, false).unwrap();
        model.process_planar(&mut audio).unwrap();
    }

    #[test]
    fn process_interleaved_variable_frames() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
        let mut audio = vec![0.0f32; 2 * 480]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480, true).unwrap();
        model.process_interleaved(&mut audio).unwrap();

        let mut audio = vec![0.0f32; 2 * 20]; // 2 channels, 20 frames
        model.process_interleaved(&mut audio).unwrap();
    }

    #[test]
    fn process_planar_variable_frames() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
        let mut left = vec![0.0f32; 480]; // 480 frames
        let mut right = vec![0.0f32; 480]; // 480 frames
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480, true).unwrap();
        model.process_planar(&mut audio).unwrap();

        let mut left = vec![0.0f32; 20]; // 20 frames
        let mut right = vec![0.0f32; 20]; // 20 frames
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()]; // 2 channels, 20 frames
        model.process_planar(&mut audio).unwrap();
    }

    #[test]
    fn process_interleaved_variable_frames_fails_without_allow_variable_frames() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
        let mut audio = vec![0.0f32; 2 * 480]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480, false).unwrap();
        model.process_interleaved(&mut audio).unwrap();

        let mut audio = vec![0.0f32; 2 * 20]; // 2 channels, 20 frames
        let result = model.process_interleaved(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }

    #[test]
    fn process_planar_variable_frames_fails_without_allow_variable_frames() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
        let mut left = vec![0.0f32; 480]; // 480 frames
        let mut right = vec![0.0f32; 480]; // 480 frames
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480, false).unwrap();
        model.process_planar(&mut audio).unwrap();

        let mut left = vec![0.0f32; 20]; // 20 frames
        let mut right = vec![0.0f32; 20]; // 20 frames
        let mut audio = [left.as_mut_slice(), right.as_mut_slice()]; // 2 channels, 20 frames
        let result = model.process_planar(&mut audio);
        assert_eq!(result, Err(AicError::AudioConfigMismatch));
    }
}
