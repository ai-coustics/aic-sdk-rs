//! Helpers shared by every test in this repository.
//!
//! Compiled twice: once as a module of the crate itself, for the unit tests in `src/`, and once as
//! a module of the `tests/end2end.rs` integration crate, which pulls this same file in with
//! `#[path]`. Both need the model-resolving helper below, and they need to share the *same*
//! instance of its lock: the test binaries run one model download at a time, so per-module locks
//! would let two modules fetch the same model concurrently.
//!
//! Because the file is compiled into two different crates it must reach the SDK through the public
//! `aic_sdk` path, which `lib.rs` aliases to the crate itself for its test builds.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

/// Resolves `model_id` to a local model file, downloading it into `target/` if it is not there yet.
///
/// Downloads happen at most once per model per process: resolution is serialized, and the results
/// are memoized so a cold cache costs one download per model rather than one per test.
pub fn test_model_path(model_id: &str) -> PathBuf {
    static RESOLVED: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();

    let mut resolved = lock(RESOLVED.get_or_init(Mutex::default));
    if let Some(path) = resolved.get(model_id) {
        return path.clone();
    }

    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");
    let path = find_existing_model(&target_dir, model_id)
        .unwrap_or_else(|| download_model(model_id, &target_dir));

    resolved.insert(model_id.to_owned(), path.clone());
    path
}

/// The license key the SDK needs to instantiate anything.
pub fn license_key() -> String {
    std::env::var("AIC_SDK_LICENSE")
        .expect("AIC_SDK_LICENSE environment variable must be set for tests")
}

/// A panicking test would otherwise poison the lock and turn one real failure into a cascade of
/// unrelated `PoisonError`s in every test that came after it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

/// Model files are named after their id with `-` and `.` replaced by `_`, followed by a hash and a
/// version suffix, so the id is a prefix of the file name.
fn find_existing_model(target_dir: &Path, model_id: &str) -> Option<PathBuf> {
    let prefix = model_id.replace(['-', '.'], "_");

    fs::read_dir(target_dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        let matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(&prefix))
            && path.extension().is_some_and(|ext| ext == "aicmodel")
            && path.is_file();

        matches.then_some(path)
    })
}

#[cfg(feature = "download-model")]
fn download_model(model_id: &str, target_dir: &Path) -> PathBuf {
    aic_sdk::Model::download(model_id, target_dir)
        .unwrap_or_else(|err| panic!("failed to download model `{model_id}`: {err}"))
}

#[cfg(not(feature = "download-model"))]
fn download_model(model_id: &str, target_dir: &Path) -> PathBuf {
    panic!(
        "model `{model_id}` not found in {} and the `download-model` feature is disabled",
        target_dir.display()
    )
}
