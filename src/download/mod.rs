use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use thiserror::Error;

mod manifest;
use manifest::Manifest;

const MODEL_BASE_URL: &str = "https://artifacts.ai-coustics.io/";

#[derive(Debug, Error)]
pub enum Error {
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
    model_id: &str,
    model_version: u32,
    download_dir: P,
) -> Result<PathBuf, Error> {
    let manifest = Manifest::download()?;
    let model = manifest.metadata_for_model(model_id, model_version)?;

    let download_dir = download_dir.as_ref();
    fs::create_dir_all(download_dir).map_err(|err| Error::Io(err.to_string()))?;

    let destination = download_dir.join(&model.file_name);
    if destination.exists() && checksum_matches(&destination, &model.checksum)? {
        return Ok(destination);
    }

    let url = format!("{MODEL_BASE_URL}{}", model.url_path);
    let bytes = download_bytes(&url)?;

    // Use a unique temporary filename to avoid race conditions when multiple threads
    // try to download the same model simultaneously
    let temp_path = download_dir.join(format!(
        "{}.{:?}.download",
        model.file_name,
        std::thread::current().id()
    ));
    fs::write(&temp_path, &bytes).map_err(|err| Error::Io(err.to_string()))?;

    if !checksum_matches(&temp_path, &model.checksum)? {
        let _ = fs::remove_file(&temp_path);
        return Err(Error::ChecksumMismatch);
    }

    // Atomic rename - if another thread already created the destination, that's fine
    match fs::rename(&temp_path, &destination) {
        Ok(()) => Ok(destination),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Another thread beat us to it, clean up our temp file
            let _ = fs::remove_file(&temp_path);
            Ok(destination)
        }
        Err(e) => Err(Error::Io(e.to_string())),
    }
}

fn download_bytes(url: &str) -> Result<Vec<u8>, Error> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| Error::ModelDownload(err.to_string()))?;

    response
        .into_body()
        .into_with_config()
        .read_to_vec()
        .map_err(|err| Error::ModelDownload(err.to_string()))
}

fn checksum_matches(path: &Path, expected: &str) -> Result<bool, Error> {
    let mut file = File::open(path).map_err(|err| Error::Io(err.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| Error::Io(err.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let checksum = format!("{:x}", hasher.finalize());
    Ok(checksum.eq_ignore_ascii_case(expected))
}
