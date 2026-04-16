use serde::Deserialize;
use std::collections::HashMap;

use super::Error;

const MANIFEST_URL: &str = "https://artifacts.ai-coustics.io/manifest.json";

#[derive(Debug, Deserialize)]
pub struct Manifest {
    models: HashMap<String, Model>,
}

#[derive(Debug, Deserialize)]
struct Model {
    versions: HashMap<String, ModelMetadata>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelMetadata {
    #[serde(rename(deserialize = "file"))]
    pub url_path: String,
    #[serde(rename(deserialize = "filename"))]
    pub file_name: String,
    pub checksum: String,
}

impl Manifest {
    pub fn download() -> Result<Self, Error> {
        let mut response = ureq::get(MANIFEST_URL)
            .call()
            .map_err(|err| Error::ManifestDownload(err.to_string()))?;

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|err| Error::ManifestDownload(err.to_string()))?;

        serde_json::from_str(&body).map_err(|err| Error::ManifestParse(err.to_string()))
    }

    pub fn metadata_for_model(&self, id: &str, version: u32) -> Result<&ModelMetadata, Error> {
        let manifest_model = self.model_entry(id)?;

        manifest_model.version(version, id)
    }

    fn model_entry(&self, id: &str) -> Result<&Model, Error> {
        self.models
            .get(id)
            .ok_or_else(|| Error::ModelNotFound(id.to_string()))
    }

    fn version_key(version: u32) -> String {
        format!("v{version}")
    }
}

impl Model {
    fn version(&self, version: u32, id: &str) -> Result<&ModelMetadata, Error> {
        self.versions
            .get(&Manifest::version_key(version))
            .ok_or_else(|| Error::IncompatibleModel {
                model: id.to_string(),
                compatible_version: version,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_manifest() -> Manifest {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/reference/manifest.json"
        )))
        .unwrap()
    }

    #[test]
    fn metadata_for_model_returns_requested_version() {
        let manifest = load_manifest();

        let model = manifest
            .metadata_for_model("quail-vf-2.0-l-16khz", 2)
            .unwrap();

        assert_eq!(
            model.file_name,
            "quail_vf_2_0_l_16khz_d42jls1e_v18.aicmodel"
        );
        assert_eq!(
            model.url_path,
            "models/quail-vf-2-0-l-16khz/v2/quail_vf_2_0_l_16khz_d42jls1e_v18.aicmodel"
        );
        assert_eq!(
            model.checksum,
            "c33a73442e2598acfd2fdc88ca127d1e8ecea0941dc93e4d3e1169246941de6e"
        );
    }
}
