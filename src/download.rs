#![cfg(feature = "download-model")]

use crate::{Model, error::*};

use aic_sdk_sys::aic_get_compatible_model_version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

impl Model {
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

#[derive(Debug, Deserialize)]
struct Manifest {
    models: HashMap<String, ManifestModel>,
}

#[derive(Debug, Deserialize)]
struct ManifestModel {
    versions: HashMap<String, ManifestVersion>,
}

#[derive(Debug, Deserialize)]
struct ManifestVersion {
    file: String,
    filename: String,
    checksum: String,
}

fn fetch_manifest(url: &str) -> Result<Manifest, AicError> {
    let response = reqwest::blocking::get(url)
        .map_err(|err| AicError::ManifestDownload(err.to_string()))?
        .error_for_status()
        .map_err(|err| AicError::ManifestDownload(err.to_string()))?;

    response
        .json::<Manifest>()
        .map_err(|err| AicError::ManifestParse(err.to_string()))
}

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
