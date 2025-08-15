use aic_sdk_sys::{AicErrorCode::*, AicModelType::*, AicParameter::*, *};
use std::ffi::{CStr, CString};
use std::ptr;
use thiserror::Error;

/// Rust-friendly error type for AIC SDK operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AicError {
    #[error("License key format is invalid or corrupted")]
    LicenseInvalid,
    #[error("License key has expired")]
    LicenseExpired,
    #[error("Audio configuration is not supported by the model")]
    UnsupportedAudioConfig,
    #[error("Process was called with a different audio buffer configuration than initialized")]
    AudioConfigMismatch,
    #[error("Model must be initialized before this operation")]
    NotInitialized,
    #[error("Parameter value is out of valid range")]
    ParameterOutOfRange,
    #[error("Unknown error code: {0}")]
    Unknown(u32),
}

impl From<core::ffi::c_uint> for AicError {
    fn from(error_code: core::ffi::c_uint) -> Self {
        match error_code {
            AIC_ERROR_CODE_NULL_POINTER => {
                // This should never happen in our Rust wrapper, but if it does,
                // it indicates a serious bug in our wrapper logic
                panic!(
                    "Unexpected null pointer error from C library - this is a bug in the Rust wrapper"
                );
            }
            AIC_ERROR_CODE_LICENSE_INVALID => AicError::LicenseInvalid,
            AIC_ERROR_CODE_LICENSE_EXPIRED => AicError::LicenseExpired,
            AIC_ERROR_CODE_UNSUPPORTED_AUDIO_CONFIG => AicError::UnsupportedAudioConfig,
            AIC_ERROR_CODE_AUDIO_CONFIG_MISMATCH => AicError::AudioConfigMismatch,
            AIC_ERROR_CODE_NOT_INITIALIZED => AicError::NotInitialized,
            AIC_ERROR_CODE_PARAMETER_OUT_OF_RANGE => AicError::ParameterOutOfRange,
            code => AicError::Unknown(code),
        }
    }
}

/// Helper function to convert C error codes to Rust Results.
pub fn handle_error(error_code: core::ffi::c_uint) -> Result<(), AicError> {
    match error_code {
        AIC_ERROR_CODE_SUCCESS => Ok(()),
        code => Err(AicError::from(code)),
    }
}

/// Supported model types for audio enhancement.
///
/// Each model type provides different levels of quality and computational requirements.
/// Quail models are the newer generation, while Legacy models are provided for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelType {
    /// Quail Large - highest quality, most computationally intensive
    QuailL,
    /// Quail Small - good balance of quality and performance
    QuailS,
    /// Quail Extra Small - optimized for lower-end devices
    QuailXS,
    /// Quail Extra Extra Small - minimal computational requirements
    QuailXXS,
    /// Legacy Large - older generation model, high quality
    LegacyL,
    /// Legacy Small - older generation model, good performance
    LegacyS,
}

impl From<ModelType> for u32 {
    fn from(model_type: ModelType) -> Self {
        match model_type {
            ModelType::QuailL => AIC_MODEL_TYPE_QUAIL_L,
            ModelType::QuailS => AIC_MODEL_TYPE_QUAIL_S,
            ModelType::QuailXS => AIC_MODEL_TYPE_QUAIL_XS,
            ModelType::QuailXXS => AIC_MODEL_TYPE_QUAIL_XXS,
            ModelType::LegacyL => AIC_MODEL_TYPE_LEGACY_L,
            ModelType::LegacyS => AIC_MODEL_TYPE_LEGACY_S,
        }
    }
}

/// Audio enhancement parameters that can be adjusted at runtime.
///
/// These parameters allow fine-tuning of the audio enhancement behavior
/// according to specific use cases and preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parameter {
    /// Enhancement level controls the strength of audio enhancement.
    /// Higher values provide more aggressive enhancement but may introduce artifacts.
    EnhancementLevel,
    /// Voice gain adjusts the amplification of voice components in the audio.
    /// Positive values increase voice prominence, negative values reduce it.
    VoiceGain,
    /// Noise gate enable controls whether background noise suppression is active.
    /// When enabled, very quiet background noise is eliminated.
    NoiseGateEnable,
}

impl From<Parameter> for u32 {
    fn from(parameter: Parameter) -> Self {
        match parameter {
            Parameter::EnhancementLevel => AIC_PARAMETER_ENHANCEMENT_LEVEL,
            Parameter::VoiceGain => AIC_PARAMETER_VOICE_GAIN,
            Parameter::NoiseGateEnable => AIC_PARAMETER_NOISE_GATE_ENABLE,
        }
    }
}

/// High-level wrapper for the AIC audio enhancement model.
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
/// let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
///
/// model.initialize(48000, 1, 1024).unwrap();
///
/// // Process audio data
/// let mut audio_buffer = vec![0.0f32; 1024];
/// model.process_interleaved(&mut audio_buffer, 1, 1024).unwrap();
/// ```
pub struct Model {
    /// Raw pointer to the C model structure
    inner: *mut AicModel,
}

impl Model {
    /// Creates a new AIC model instance of the specified type.
    ///
    /// # Arguments
    ///
    /// * `model_type` - The type of model to create
    /// * `license_key` - Valid license key for the AIC SDK
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Model` instance or an `Error` if creation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// let model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// ```
    pub fn new(model_type: ModelType, license_key: &str) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_license_key = CString::new(license_key).map_err(|_| AicError::LicenseInvalid)?;

        let error_code =
            unsafe { aic_model_create(&mut model_ptr, model_type.into(), c_license_key.as_ptr()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self { inner: model_ptr })
    }

    /// Initializes the model with the specified audio configuration.
    ///
    /// This must be called before any audio processing can occur.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Audio sample rate in Hz (e.g., 48000)
    /// * `num_channels` - Number of audio channels (typically 1 for mono, 2 for stereo)
    /// * `num_frames` - Number of frames per processing block
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `Error` if initialization fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// model.initialize(48000, 1, 1024).unwrap();
    /// ```
    pub fn initialize(
        &mut self,
        sample_rate: u32,
        num_channels: u16,
        num_frames: usize,
    ) -> Result<(), AicError> {
        let error_code =
            unsafe { aic_model_initialize(self.inner, sample_rate, num_channels, num_frames) };

        handle_error(error_code)?;
        Ok(())
    }

    /// Resets the internal state of the model.
    ///
    /// This clears any internal buffers and history, returning the model
    /// to its initial state. The model remains initialized and configured.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `Error` if the reset fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// model.reset().unwrap();
    /// ```
    pub fn reset(&mut self) -> Result<(), AicError> {
        let error_code = unsafe { aic_model_reset(self.inner) };
        handle_error(error_code)
    }

    /// Processes planar audio data (separate arrays for each channel) in-place.
    ///
    /// In planar format, each channel's data is stored in a separate contiguous array.
    /// For stereo audio, you would have two separate arrays: one for left channel
    /// and one for right channel. The audio is enhanced in-place.
    ///
    /// # Arguments
    ///
    /// * `audio` - Mutable slice of audio channel buffers to be enhanced in-place
    /// * `num_channels` - Number of audio channels
    /// * `num_frames` - Number of frames to process
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `Error` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// let mut audio = vec![vec![0.0f32; 480]; 2]; // 2 channels, 480 frames each
    /// let mut audio_refs: Vec<&mut [f32]> = audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
    /// model.initialize(48000, 2, 480).unwrap();
    /// model.process_planar(&mut audio_refs).unwrap();
    /// ```
    pub fn process_planar(&mut self, audio: &mut [&mut [f32]]) -> Result<(), AicError> {
        const MAX_CHANNELS: usize = 16;

        let num_channels = audio.len() as u16;
        let num_frames = if audio.is_empty() { 0 } else { audio[0].len() };

        let mut audio_ptrs = [std::ptr::null_mut::<f32>(); MAX_CHANNELS];
        for (i, channel) in audio.iter_mut().enumerate().take(MAX_CHANNELS) {
            audio_ptrs[i] = channel.as_mut_ptr();
        }

        dbg!(num_channels);
        dbg!(num_frames);

        let error_code = unsafe {
            aic_model_process_planar(self.inner, audio_ptrs.as_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }

    /// Processes interleaved audio data (samples from all channels mixed together) in-place.
    ///
    /// In interleaved format, samples from different channels are stored alternately.
    /// For stereo audio: [L1, R1, L2, R2, L3, R3, ...] where L = left, R = right.
    /// The audio is enhanced in-place.
    ///
    /// # Arguments
    ///
    /// * `audio` - Mutable slice of interleaved audio samples to be enhanced in-place
    /// * `num_channels` - Number of audio channels
    /// * `num_frames` - Number of frames to process
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `Error` if processing fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// let mut audio = vec![0.0f32; 2 * 480]; // 2 channels, 480 frames
    /// model.initialize(48000, 2, 480).unwrap();
    /// model.process_interleaved(&mut audio, 2, 480).unwrap();
    /// ```
    pub fn process_interleaved(
        &mut self,
        audio: &mut [f32],
        num_channels: u16,
        num_frames: usize,
    ) -> Result<(), AicError> {
        let error_code = unsafe {
            aic_model_process_interleaved(self.inner, audio.as_mut_ptr(), num_channels, num_frames)
        };

        handle_error(error_code)
    }

    /// Sets a parameter value for the model.
    ///
    /// Parameters control various aspects of the audio enhancement behavior
    /// and can be adjusted in real-time during processing.
    ///
    /// # Arguments
    ///
    /// * `parameter` - The parameter to set
    /// * `value` - The new value for the parameter
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success or an `Error` if the parameter cannot be set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType, Parameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// model.set_parameter(Parameter::EnhancementLevel, 0.8).unwrap();
    /// model.set_parameter(Parameter::NoiseGateEnable, 1.0).unwrap(); // 1.0 = enabled
    /// ```
    pub fn set_parameter(&mut self, parameter: Parameter, value: f32) -> Result<(), AicError> {
        let error_code = unsafe { aic_model_set_parameter(self.inner, parameter.into(), value) };
        handle_error(error_code)
    }

    /// Gets the current value of a parameter.
    ///
    /// # Arguments
    ///
    /// * `parameter` - The parameter to query
    ///
    /// # Returns
    ///
    /// Returns `Ok(value)` containing the current parameter value, or an `Error` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType, Parameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// let enhancement_level = model.get_parameter(Parameter::EnhancementLevel).unwrap();
    /// println!("Current enhancement level: {}", enhancement_level);
    /// ```
    pub fn get_parameter(&self, parameter: Parameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        let error_code =
            unsafe { aic_model_get_parameter(self.inner, parameter.into(), &mut value) };
        handle_error(error_code)?;
        Ok(value)
    }

    /// Gets the processing latency for this model instance.
    ///
    /// The latency represents the delay introduced by the audio processing in samples.
    /// This can be useful for applications that need to compensate for processing delay.
    ///
    /// # Returns
    ///
    /// Returns `Ok(latency_samples)` or an `Error` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// let latency = model.processing_latency().unwrap();
    /// println!("Processing latency: {} samples", latency);
    /// ```
    pub fn processing_latency(&self) -> Result<usize, AicError> {
        let mut latency: usize = 0;
        let error_code = unsafe { aic_get_processing_latency(self.inner, &mut latency) };
        handle_error(error_code)?;
        Ok(latency)
    }

    /// Gets the optimal sample rate for this model instance.
    ///
    /// Using the optimal sample rate can improve both quality and performance.
    /// While other sample rates may be supported, the optimal rate is recommended
    /// for best results.
    ///
    /// # Returns
    ///
    /// Returns `Ok(sample_rate)` or an `Error` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// let optimal_rate = model.optimal_sample_rate().unwrap();
    /// println!("Optimal sample rate: {} Hz", optimal_rate);
    /// ```
    pub fn optimal_sample_rate(&self) -> Result<u32, AicError> {
        let mut sample_rate: u32 = 0;
        let error_code = unsafe { aic_get_optimal_sample_rate(self.inner, &mut sample_rate) };
        handle_error(error_code)?;
        Ok(sample_rate)
    }

    /// Gets the optimal number of frames per processing block for this model instance.
    ///
    /// Using the optimal frame count can improve processing efficiency and reduce latency.
    /// The frame count determines how many audio samples are processed in each call
    /// to the processing functions.
    ///
    /// # Returns
    ///
    /// Returns `Ok(num_frames)` or an `Error` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use aic_sdk::{Model, ModelType};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
    /// let optimal_frames = model.optimal_num_frames().unwrap();
    /// println!("Optimal frame count: {}", optimal_frames);
    /// ```
    pub fn optimal_num_frames(&self) -> Result<usize, AicError> {
        let mut num_frames: usize = 0;
        let error_code = unsafe { aic_get_optimal_num_frames(self.inner, &mut num_frames) };
        handle_error(error_code)?;
        Ok(num_frames)
    }
}

impl Drop for Model {
    /// Automatically cleans up the model when it goes out of scope.
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

/// Gets the version string of the underlying C library.
///
/// # Returns
///
/// Returns the library version as a string, or `None` if the version cannot be retrieved.
///
/// # Example
///
/// ```rust
/// # use aic_sdk::aic_sdk_version;
/// if let Some(version) = aic_sdk_version() {
///     println!("AIC SDK version: {}", version);
/// }
/// ```
pub fn aic_sdk_version() -> Option<String> {
    let version_ptr = unsafe { aic_get_library_version() };
    if version_ptr.is_null() {
        return None;
    }

    unsafe {
        CStr::from_ptr(version_ptr)
            .to_str()
            .ok()
            .map(|s| s.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_creation_and_basic_operations() -> Result<(), AicError> {
        dbg!(aic_sdk_version());

        // Read license key from environment variable
        let license_key = std::env::var("AIC_SDK_LICENSE")
            .expect("AIC_SDK_LICENSE environment variable must be set for tests");

        // Test model creation with QuailL at optimal settings
        let mut model = Model::new(ModelType::QuailL, &license_key)?;

        // Test initialization with QuailL optimal settings (48000 Hz, 480 frames)
        model.initialize(48000, 2, 480)?;

        let mut audio = vec![vec![0.0f32; 480]; 2]; // 2 channels, 480 frames each
        let mut audio_refs: Vec<&mut [f32]> =
            audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();

        model.process_planar(&mut audio_refs).unwrap();

        Ok(())
    }

    #[test]
    fn processing() {
        let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
        let mut model = Model::new(ModelType::QuailS, &license_key).unwrap();
        let mut audio = vec![0.0f32; 2 * 480]; // 2 channels, 480 frames
        model.initialize(48000, 2, 480).unwrap();
        model.process_interleaved(&mut audio, 2, 480).unwrap();
    }
}
