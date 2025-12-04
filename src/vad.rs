use crate::error::*;

use aic_sdk_sys::{AicVadParameter::*, *};

/// Configurable parameters for Voice Activity Detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VadParameter {
    /// Controls the lookback buffer size used in the Voice Activity Detector.
    ///
    /// The lookback buffer size is the number of window-length audio buffers
    /// the VAD has available as a lookback buffer.
    ///
    /// The stability of the prediction increases with the buffer size,
    /// at the cost of higher latency.
    ///
    /// **Range:** 1.0 to 20.0
    ///
    /// **Default:** 6.0
    LookbackBufferSize,
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
}

impl From<VadParameter> for AicVadParameter::Type {
    fn from(parameter: VadParameter) -> Self {
        match parameter {
            VadParameter::LookbackBufferSize => AIC_VAD_PARAMETER_LOOKBACK_BUFFER_SIZE,
            VadParameter::Sensitivity => AIC_VAD_PARAMETER_SENSITIVITY,
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
/// ```rust
/// use aic_sdk::{Model, ModelType};
///
/// let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
/// let mut model = Model::new(ModelType::QuailS48, &license_key).unwrap();
/// let vad = model.create_vad();
/// ```
pub struct Vad {
    /// Raw pointer to the C VAD structure
    inner: *mut AicVad,
}

impl Vad {
    /// Creates a new VAD instance.
    pub(crate) fn new(vad_ptr: *mut AicVad) -> Self {
        Self { inner: vad_ptr }
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
        let error_code = unsafe { aic_vad_is_speech_detected(self.inner, &mut value) };

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
    /// ```rust
    /// # use aic_sdk::{Model, ModelType, VadParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// # let mut vad = model.create_vad();
    /// vad.set_parameter(VadParameter::LookbackBufferSize, 10.0).unwrap();
    /// vad.set_parameter(VadParameter::Sensitivity, 5.0).unwrap();
    /// ```
    pub fn set_parameter(&mut self, parameter: VadParameter, value: f32) -> Result<(), AicError> {
        let error_code = unsafe { aic_vad_set_parameter(self.inner, parameter.into(), value) };
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
    /// ```rust
    /// # use aic_sdk::{Model, ModelType, VadParameter};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::new(ModelType::QuailS48, &license_key).unwrap();
    /// # let vad = model.create_vad();
    /// let sensitivity = vad.parameter(VadParameter::Sensitivity).unwrap();
    /// println!("Current sensitivity: {sensitivity}");
    /// ```
    pub fn parameter(&self, parameter: VadParameter) -> Result<f32, AicError> {
        let mut value: f32 = 0.0;
        let error_code = unsafe { aic_vad_get_parameter(self.inner, parameter.into(), &mut value) };
        handle_error(error_code)?;
        Ok(value)
    }
}

impl Drop for Vad {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe {
                aic_vad_destroy(self.inner);
            }
        }
    }
}

// Safety: The underlying C library should be thread-safe for individual VAD instances
unsafe impl Send for Vad {}
unsafe impl Sync for Vad {}
