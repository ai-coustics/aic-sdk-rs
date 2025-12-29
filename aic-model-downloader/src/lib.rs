use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use thiserror::Error;

const MODEL_BASE_URL: &str = "https://d3lqwskupyztjd.cloudfront.net/";
const MANIFEST_URL: &str = "https://d3lqwskupyztjd.cloudfront.net/manifest.json";

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Failed to download manifest: {0}")]
    ManifestDownload(String),
    #[error("Failed to parse manifest: {0}")]
    ManifestParse(String),
    #[error("Model `{0}` not found in manifest")]
    ModelNotFound(String),
    #[error("Model `{model}` missing compatible version v{compatible_version}")]
    IncompatibleModel {
        model: String,
        compatible_version: u32,
    },
    #[error("Failed to download model file: {0}")]
    ModelDownload(String),
    #[error("Checksum mismatch for downloaded model")]
    ChecksumMismatch,
}

/// Downloads a model file compatible with the provided model version.
///
/// The function fetches the model manifest, checks whether the requested model
/// exists in a version compatible with the given `model_version`, and downloads
/// the model file into the provided directory.
pub fn download<P: AsRef<Path>>(
    model: &str,
    model_version: u32,
    download_dir: P,
) -> Result<PathBuf, DownloadError> {
    let manifest = fetch_manifest(MANIFEST_URL)?;
    let version_key = format!("v{model_version}");

    let manifest_model = manifest
        .models
        .get(model)
        .ok_or_else(|| DownloadError::ModelNotFound(model.to_string()))?;

    let version = manifest_model.versions.get(&version_key).ok_or_else(|| {
        DownloadError::IncompatibleModel {
            model: model.to_string(),
            compatible_version: model_version,
        }
    })?;

    let download_dir = download_dir.as_ref();
    fs::create_dir_all(download_dir).map_err(|err| DownloadError::Io(err.to_string()))?;

    let destination = download_dir.join(&version.filename);
    if destination.exists() && checksum_matches(&destination, &version.checksum)? {
        return Ok(destination);
    }

    let url = format!("{MODEL_BASE_URL}{}", version.file);
    let bytes = download_bytes(&url)?;

    let temp_path = destination.with_extension("download");
    fs::write(&temp_path, &bytes).map_err(|err| DownloadError::Io(err.to_string()))?;

    if !checksum_matches(&temp_path, &version.checksum)? {
        let _ = fs::remove_file(&temp_path);
        return Err(DownloadError::ChecksumMismatch);
    }

    fs::rename(&temp_path, &destination).map_err(|err| DownloadError::Io(err.to_string()))?;

    Ok(destination)
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

fn fetch_manifest(url: &str) -> Result<Manifest, DownloadError> {
    let response = reqwest::blocking::get(url)
        .map_err(|err| DownloadError::ManifestDownload(err.to_string()))?
        .error_for_status()
        .map_err(|err| DownloadError::ManifestDownload(err.to_string()))?;

    response
        .json::<Manifest>()
        .map_err(|err| DownloadError::ManifestParse(err.to_string()))
}

fn download_bytes(url: &str) -> Result<Vec<u8>, DownloadError> {
    let response = reqwest::blocking::get(url)
        .map_err(|err| DownloadError::ModelDownload(err.to_string()))?
        .error_for_status()
        .map_err(|err| DownloadError::ModelDownload(err.to_string()))?;

    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|err| DownloadError::ModelDownload(err.to_string()))
}

fn checksum_matches(path: &Path, expected: &str) -> Result<bool, DownloadError> {
    let mut file = File::open(path).map_err(|err| DownloadError::Io(err.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| DownloadError::Io(err.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let checksum = format!("{:x}", hasher.finalize());
    Ok(checksum.eq_ignore_ascii_case(expected))
}
