use crate::error::*;

use aic_sdk_sys::*;

use std::{ffi::CString, path::Path, ptr};

#[cfg(feature = "download-model")]
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::PathBuf,
};

#[cfg(feature = "download-model")]
use serde::Deserialize;
#[cfg(feature = "download-model")]
use sha2::{Digest, Sha256};

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
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        let c_path = CString::new(path.as_ref().to_string_lossy().as_bytes()).unwrap();
        
        let error_code =
            unsafe { aic_model_create_from_file(&mut model_ptr, c_path.as_ptr()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: model_ptr,
        })
    }

    pub fn from_buffer(buffer: &[u8]) -> Result<Self, AicError> {
        let mut model_ptr: *mut AicModel = ptr::null_mut();
        
        let error_code =
            unsafe { aic_model_create_from_buffer(&mut model_ptr, buffer.as_ptr(), buffer.len()) };

        handle_error(error_code)?;

        // This should never happen if the C library is well-behaved, but let's be defensive
        assert!(
            !model_ptr.is_null(),
            "C library returned success but null pointer"
        );

        Ok(Self {
            inner: model_ptr,
        })
    }

    pub(crate) fn as_const_ptr(&self) -> *const AicModel {
        self.inner as *const AicModel
    }

    /// Downloads a model file compatible with the current SDK version.
    ///
    /// This method fetches the model manifest, checks whether the requested model
    /// exists in a version compatible with this library, and downloads the model
    /// file into the provided directory.
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier as listed in the manifest (e.g. `"quail-l-16khz"`).
    /// * `download_dir` - Directory where the downloaded model file should be stored.
    ///
    /// # Returns
    ///
    /// Returns the full path to the downloaded model file, or an `AicError` if the
    /// operation fails.
    #[cfg(feature = "download-model")]
    pub fn download<P: AsRef<Path>>(model: &str, download_dir: P) -> Result<PathBuf, AicError> {
        const MANIFEST_URL: &str = "https://d3lqwskupyztjd.cloudfront.net/manifest.json";
        const MODEL_BASE_URL: &str = "https://d3lqwskupyztjd.cloudfront.net/";

        let manifest = fetch_manifest(MANIFEST_URL)?;
        let compatible_version = unsafe { aic_get_compatible_model_version() };
        let version_key = format!("v{compatible_version}");

        let manifest_model = manifest
            .models
            .get(model)
            .ok_or_else(|| AicError::ModelNotFound(model.to_string()))?;

        let version = manifest_model.versions.get(&version_key).ok_or_else(|| {
            AicError::IncompatibleModel {
                model: model.to_string(),
                compatible_version,
            }
        })?;

        let download_dir = download_dir.as_ref();
        fs::create_dir_all(download_dir).map_err(|err| AicError::Io(err.to_string()))?;

        let destination = download_dir.join(&version.filename);
        if destination.exists() && checksum_matches(&destination, &version.checksum)? {
            return Ok(destination);
        }

        let url = format!("{MODEL_BASE_URL}{}", version.file);
        let bytes = download_bytes(&url)?;

        let temp_path = destination.with_extension("download");
        fs::write(&temp_path, &bytes).map_err(|err| AicError::Io(err.to_string()))?;

        if !checksum_matches(&temp_path, &version.checksum)? {
            let _ = fs::remove_file(&temp_path);
            return Err(AicError::ChecksumMismatch);
        }

        fs::rename(&temp_path, &destination).map_err(|err| AicError::Io(err.to_string()))?;

        Ok(destination)
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

// SAFETY: The Model struct can be safely sent and shared between threads
unsafe impl Send for Model {}
unsafe impl Sync for Model {}

#[cfg(feature = "download-model")]
#[derive(Debug, Deserialize)]
struct Manifest {
    models: HashMap<String, ManifestModel>,
}

#[cfg(feature = "download-model")]
#[derive(Debug, Deserialize)]
struct ManifestModel {
    versions: HashMap<String, ManifestVersion>,
}

#[cfg(feature = "download-model")]
#[derive(Debug, Deserialize)]
struct ManifestVersion {
    file: String,
    filename: String,
    checksum: String,
}

#[cfg(feature = "download-model")]
fn fetch_manifest(url: &str) -> Result<Manifest, AicError> {
    let response = reqwest::blocking::get(url)
        .map_err(|err| AicError::ManifestDownload(err.to_string()))?
        .error_for_status()
        .map_err(|err| AicError::ManifestDownload(err.to_string()))?;

    response
        .json::<Manifest>()
        .map_err(|err| AicError::ManifestParse(err.to_string()))
}

#[cfg(feature = "download-model")]
fn download_bytes(url: &str) -> Result<Vec<u8>, AicError> {
    let response = reqwest::blocking::get(url)
        .map_err(|err| AicError::ModelDownload(err.to_string()))?
        .error_for_status()
        .map_err(|err| AicError::ModelDownload(err.to_string()))?;

    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|err| AicError::ModelDownload(err.to_string()))
}

#[cfg(feature = "download-model")]
fn checksum_matches(path: &Path, expected: &str) -> Result<bool, AicError> {
    let mut file = File::open(path).map_err(|err| AicError::Io(err.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| AicError::Io(err.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let checksum = format!("{:x}", hasher.finalize());
    Ok(checksum.eq_ignore_ascii_case(expected))
}
