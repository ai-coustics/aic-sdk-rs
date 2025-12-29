use thiserror::Error;

use aic_sdk_sys::AicErrorCode::{self, *};

/// Error type for AIC SDK operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AicError {
    #[error("Parameter is out of range")]
    ParameterOutOfRange,
    #[error("Processor was not initialized")]
    ModelNotInitialized,
    #[error("Audio config is not supported")]
    AudioConfigUnsupported,
    #[error("Audio config does not match the initialized config")]
    AudioConfigMismatch,
    #[error("Audio enhancement was disallowed")]
    EnhancementNotAllowed,
    #[error("Internal error")]
    Internal,
    #[error("Parameter can't be changed for this model type")]
    ParameterFixed,
    #[error("License key is invalid")]
    LicenseFormatInvalid,
    #[error("License version unsupported")]
    LicenseVersionUnsupported,
    #[error("License key expired")]
    LicenseExpired,
    #[error("Model download error: {0}")]
    ModelDownload(String),
    #[error("Unknown error code: {0}")]
    Unknown(AicErrorCode::Type),
}

impl From<AicErrorCode::Type> for AicError {
    fn from(error_code: AicErrorCode::Type) -> Self {
        match error_code {
            AIC_ERROR_CODE_NULL_POINTER => {
                // This should never happen in our Rust wrapper, but if it does,
                // it indicates a serious bug in our wrapper logic
                panic!(
                    "Unexpected null pointer error from C library - this is a bug in the Rust wrapper"
                );
            }
            AIC_ERROR_CODE_PARAMETER_OUT_OF_RANGE => AicError::ParameterOutOfRange,
            AIC_ERROR_CODE_MODEL_NOT_INITIALIZED => AicError::ModelNotInitialized,
            AIC_ERROR_CODE_AUDIO_CONFIG_UNSUPPORTED => AicError::AudioConfigUnsupported,
            AIC_ERROR_CODE_AUDIO_CONFIG_MISMATCH => AicError::AudioConfigMismatch,
            AIC_ERROR_CODE_ENHANCEMENT_NOT_ALLOWED => AicError::EnhancementNotAllowed,
            AIC_ERROR_CODE_INTERNAL_ERROR => AicError::Internal,
            AIC_ERROR_CODE_PARAMETER_FIXED => AicError::ParameterFixed,
            AIC_ERROR_CODE_LICENSE_FORMAT_INVALID => AicError::LicenseFormatInvalid,
            AIC_ERROR_CODE_LICENSE_VERSION_UNSUPPORTED => AicError::LicenseVersionUnsupported,
            AIC_ERROR_CODE_LICENSE_EXPIRED => AicError::LicenseExpired,
            code => AicError::Unknown(code),
        }
    }
}

/// Helper function to convert C error codes into Result.
pub(crate) fn handle_error(error_code: AicErrorCode::Type) -> Result<(), AicError> {
    match error_code {
        AIC_ERROR_CODE_SUCCESS => Ok(()),
        code => Err(AicError::from(code)),
    }
}
