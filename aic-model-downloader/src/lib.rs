use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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

    let temp_path = temp_path_for(&destination);
    fs::write(&temp_path, &bytes).map_err(|err| Error::Io(err.to_string()))?;

    if !checksum_matches(&temp_path, &model.checksum)? {
        let _ = fs::remove_file(&temp_path);
        return Err(Error::ChecksumMismatch);
    }

    // Another downloader may have finished the same model while we were fetching it. Its file is
    // byte-identical, so adopt it rather than replacing it: on Windows the rename below would fail
    // outright if that file is already open, and there is nothing to gain from the write.
    if destination.exists() && checksum_matches(&destination, &model.checksum)? {
        let _ = fs::remove_file(&temp_path);
        return Ok(destination);
    }

    if let Err(err) = fs::rename(&temp_path, &destination) {
        // Lost the same race a moment later, between the check above and the rename.
        if destination.exists() && checksum_matches(&destination, &model.checksum)? {
            let _ = fs::remove_file(&temp_path);
            return Ok(destination);
        }
        let _ = fs::remove_file(&temp_path);
        return Err(Error::Io(err.to_string()));
    }

    Ok(destination)
}

/// Staging path for a download, unique per call.
///
/// Concurrent downloads of the same model must not share a staging file: they would interleave
/// their writes, and whoever renamed second would find the file already gone. The pid keeps
/// separate processes apart, the counter separate threads, and the suffix is appended rather than
/// substituted so the model's own extension stays visible in the temporary name.
fn temp_path_for(destination: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let file_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);

    destination.with_file_name(format!(
        "{file_name}.{}.{unique}.download",
        std::process::id()
    ))
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

    let checksum = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(checksum.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_paths_are_unique_per_call() {
        let destination = Path::new("/models/quail_vf_2_2_s_16khz_abcd1234_v12.aicmodel");

        let first = temp_path_for(destination);
        let second = temp_path_for(destination);

        assert_ne!(first, second);
        assert_eq!(first.parent(), destination.parent());
        assert_eq!(second.parent(), destination.parent());
    }

    #[test]
    fn temp_path_keeps_the_model_file_name() {
        let destination = Path::new("/models/quail_vf_2_2_s_16khz_abcd1234_v12.aicmodel");

        let temp = temp_path_for(destination);
        let name = temp.file_name().unwrap().to_str().unwrap();

        assert!(
            name.starts_with("quail_vf_2_2_s_16khz_abcd1234_v12.aicmodel."),
            "unexpected staging name: {name}"
        );
        assert!(
            name.ends_with(".download"),
            "unexpected staging name: {name}"
        );
    }
}
