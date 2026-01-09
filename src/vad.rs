use crate::error::*;

use aic_sdk_sys::{AicVadParameter::*, *};

/// Configurable parameters for Voice Activity Detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VadParameter {
    /// Controls for how long the VAD continues to detect speech after the audio signal
    /// no longer contains speech.
    ///
    /// The VAD reports speech detected if the audio signal contained speech in at least 50%
    /// of the frames processed in the last `speech_hold_duration` seconds.
    ///
    /// This affects the stability of speech detected -> not detected transitions.
    ///
    /// NOTE: The VAD returns a value per processed buffer, so this duration is rounded
    /// to the closest model window length. For example, if the model has a processing window
    /// length of 10 ms, the VAD will round up/down to the closest multiple of 10 ms.
    /// Because of this, this parameter may return a different value than the one it was last set to.
    ///
    /// **Range:** 0.0 to 20x model window length (value in seconds)
    ///
    /// **Default:** 0.05 (50 ms)
    SpeechHoldDuration,
    /// Controls the sensitivity (energy threshold) of the VAD.
    ///
    /// This value is used by the VAD as the threshold a
    /// speech audio signal's energy has to exceed in order to be
    /// considered speech.
    ///
    /// **Range:** 1.0 to 15.0
    ///
    /// **Formula:** Energy threshold = 10 ^ (-sensitivity)
    ///
    /// **Default:** 6.0
    Sensitivity,
    /// Controls for how long speech needs to be present in the audio signal before
    /// the VAD considers it speech.
    ///
    /// This affects the stability of speech not detected -> detected transitions.
    ///
    /// NOTE: The VAD returns a value per processed buffer, so this duration is rounded
    /// to the closest model window length. For example, if the model has a processing window
    /// length of 10 ms, the VAD will round up/down to the closest multiple of 10 ms.
    /// Because of this, this parameter may return a different value than the one it was last set to.
    ///
    /// **Range:** 0.0 to 1.0 (value in seconds)
    ///
    /// **Default:** 0.0
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

/// Voice Activity Detector backed by an ai-coustics speech enhancement model.
///
/// The VAD works automatically using the enhanced audio output of the model
/// that created the VAD.
///
/// **Important:** If the backing model is destroyed, the VAD instance will stop
/// producing new data.
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Model, Processor};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let model = Model::from_file("/path/to/model.aicmodel").unwrap();
/// let processor = Processor::new(&model, &license_key).unwrap();
/// let vad = processor.vad_context();
/// ```
pub struct VadContext {
    /// Raw pointer to the C VAD structure
    inner: *mut AicVadContext,
}

impl VadContext {
    /// Creates a new VAD context.
    pub(crate) fn new(vad_ptr: *mut AicVadContext) -> Self {
        Self { inner: vad_ptr }
    }

    fn as_const_ptr(&self) -> *const AicVadContext {
        self.inner as *const AicVadContext
    }

    /// Returns the VAD's prediction.
    ///
    /// **Important:**
    /// - The latency of the VAD prediction is equal to
    ///   the backing model's processing latency.
    /// - If the backing model stops being processed,
    ///   the VAD will not update its speech detection prediction.
    pub fn is_speech_detected(&self) -> bool {
        let mut value: bool = false;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD.
        // - `value` points to stack storage for output.
        let error_code =
            unsafe { aic_vad_context_is_speech_detected(self.as_const_ptr(), &mut value) };

        // This should never fail
        assert!(handle_error(error_code).is_ok());
        value
    }

    /// Modifies a VAD parameter.
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
    /// # use aic_sdk::{Model, Processor, VadParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let vad_context = processor.vad_context();
    /// vad_context.set_parameter(VadParameter::SpeechHoldDuration, 0.08).unwrap();
    /// vad_context.set_parameter(VadParameter::Sensitivity, 5.0).unwrap();
    /// ```
    pub fn set_parameter(&self, parameter: VadParameter, value: f32) -> Result<(), AicError> {
        // SAFETY:
        // - `self.as_const_ptr()` is a live VAD pointer.
        let error_code =
            unsafe { aic_vad_context_set_parameter(self.as_const_ptr(), parameter.into(), value) };
        handle_error(error_code)
    }

    /// Retrieves the current value of a VAD parameter.
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
    /// # use aic_sdk::{Model, Processor, VadParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel").unwrap();
    /// # let processor = Processor::new(&model, &license_key).unwrap();
    /// # let vad_context = processor.vad_context();
    /// let sensitivity = vad_context.parameter(VadParameter::Sensitivity).unwrap();
    /// println!("Current sensitivity: {sensitivity}");
    /// ```
    pub fn parameter(&self, parameter: VadParameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        // SAFETY:
        // - `self.as_const_ptr()` is a valid pointer to a live VAD.
        // - `value` points to stack storage for output.
        let error_code = unsafe {
            aic_vad_context_get_parameter(self.as_const_ptr(), parameter.into(), &mut value)
        };
        handle_error(error_code)?;
        Ok(value)
    }
}

impl Drop for VadContext {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            // SAFETY:
            // - `self.inner` was allocated by the SDK and is still owned by this wrapper.
            unsafe { aic_vad_context_destroy(self.inner) };
        }
    }
}

// Safety: The underlying C library should be thread-safe for individual VAD instances
unsafe impl Send for VadContext {}
unsafe impl Sync for VadContext {}
