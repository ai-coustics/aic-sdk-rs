use thiserror::Error;

use aic_sdk_sys::AicErrorCode::{self, *};

/// Error type for AIC SDK operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AicError {
    #[error(
        "Parameter value is outside the acceptable range. Check documentation for valid values."
    )]
    ParameterOutOfRange,
    #[error(
        "Processor must be initialized before calling this operation. Call `Processor::initialize` first."
    )]
    ProcessorNotInitialized,
    #[error(
        "Audio configuration (samplerate, num_channels, num_frames) is not supported by the model"
    )]
    AudioConfigUnsupported,
    #[error("Audio buffer configuration differs from the one provided during initialization")]
    AudioConfigMismatch,
    #[error(
        "SDK key was not authorized or process failed to report usage. Check if you have internet connection."
    )]
    EnhancementNotAllowed,
    #[error("Internal error occurred. Contact support.")]
    Internal,
    #[error("The requested parameter is read-only for this model type and cannot be modified.")]
    ParameterFixed,
    #[error("License key format is invalid or corrupted. Verify the key was copied correctly.")]
    LicenseFormatInvalid,
    #[error(
        "License version is not compatible with the SDK version. Update SDK or contact support."
    )]
    LicenseVersionUnsupported,
    #[error("License key has expired. Renew your license to continue.")]
    LicenseExpired,
    #[error("The model file is invalid or corrupted. Verify the file is correct.")]
    ModelInvalid,
    #[error("The model file version is not compatible with this SDK version.")]
    ModelVersionUnsupported,
    #[error("The path to the model file is invalid")]
    ModelFilePathInvalid,
    #[error(
        "The model file cannot be opened due to a filesystem error. Verify that the file exists."
    )]
    FileSystemError,
    #[error("The model data is not aligned to 64 bytes.")]
    ModelDataUnaligned,
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
            AIC_ERROR_CODE_PROCESSOR_NOT_INITIALIZED => AicError::ProcessorNotInitialized,
            AIC_ERROR_CODE_AUDIO_CONFIG_UNSUPPORTED => AicError::AudioConfigUnsupported,
            AIC_ERROR_CODE_AUDIO_CONFIG_MISMATCH => AicError::AudioConfigMismatch,
            AIC_ERROR_CODE_ENHANCEMENT_NOT_ALLOWED => AicError::EnhancementNotAllowed,
            AIC_ERROR_CODE_INTERNAL_ERROR => AicError::Internal,
            AIC_ERROR_CODE_PARAMETER_FIXED => AicError::ParameterFixed,
            AIC_ERROR_CODE_LICENSE_FORMAT_INVALID => AicError::LicenseFormatInvalid,
            AIC_ERROR_CODE_LICENSE_VERSION_UNSUPPORTED => AicError::LicenseVersionUnsupported,
            AIC_ERROR_CODE_LICENSE_EXPIRED => AicError::LicenseExpired,
            AIC_ERROR_CODE_MODEL_INVALID => AicError::ModelInvalid,
            AIC_ERROR_CODE_MODEL_VERSION_UNSUPPORTED => AicError::ModelVersionUnsupported,
            AIC_ERROR_CODE_MODEL_FILE_PATH_INVALID => AicError::ModelFilePathInvalid,
            AIC_ERROR_CODE_FILE_SYSTEM_ERROR => AicError::FileSystemError,
            AIC_ERROR_CODE_MODEL_DATA_UNALIGNED => AicError::ModelDataUnaligned,
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

pub(crate) fn assert_success(error_code: AicErrorCode::Type, message: &str) {
    assert_eq!(error_code, AIC_ERROR_CODE_SUCCESS, "{}", message);
}
