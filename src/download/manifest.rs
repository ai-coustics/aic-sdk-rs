use serde::Deserialize;
use std::collections::HashMap;

use super::Error;

const MANIFEST_URL: &str = "https://d3lqwskupyztjd.cloudfront.net/manifest.json";

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
        let response = reqwest::blocking::get(MANIFEST_URL)
            .map_err(|err| Error::ManifestDownload(err.to_string()))?
            .error_for_status()
            .map_err(|err| Error::ManifestDownload(err.to_string()))?;

        response
            .json::<Manifest>()
            .map_err(|err| Error::ManifestParse(err.to_string()))
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
    use std::collections::HashSet;

    fn load_manifest() -> Manifest {
        serde_json::from_str(include_str!("../reference/manifest.json")).unwrap()
    }

    #[test]
    fn metadata_for_model_returns_requested_version() {
        let manifest = load_manifest();

        let model = manifest.metadata_for_model("quail-xxs-48khz", 1).unwrap();

        assert_eq!(model.file_name, "quail_xxs_48khz_wsur2zkw_v7.aicmodel");
        assert_eq!(
            model.url_path,
            "models/quail-xxs-48khz/v1/quail_xxs_48khz_wsur2zkw_v7.aicmodel"
        );
        assert_eq!(
            model.checksum,
            "fc536364e0b6e851a37ad9790721ee2368ba06e761a78485fabbd6629d6c4cf8"
        );
    }
}
