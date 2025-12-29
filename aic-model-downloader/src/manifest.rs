use serde::Deserialize;
use std::collections::HashMap;

use crate::Error;

const MANIFEST_URL: &str = "https://d3lqwskupyztjd.cloudfront.net/manifest.json";

#[derive(Debug, Deserialize)]
pub struct Manifest {
    models: HashMap<String, ManifestModel>,
}

#[derive(Debug, Deserialize)]
struct ManifestModel {
    versions: HashMap<String, Model>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Model {
    #[serde(rename(deserialize = "file"))]
    pub url_path: String,
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

    pub fn model(
        &self,
        id: &str,
        version: u32,
    ) -> Result<&Model, Error> {
        let manifest_model = self
            .models
            .get(id)
            .ok_or_else(|| Error::ModelNotFound(id.to_string()))?;

        let version_key = format!("v{version}");

        manifest_model
            .versions
            .get(&version_key)
            .ok_or_else(|| Error::IncompatibleModel {
                model: id.to_string(),
                compatible_version: version,
            })
    }
}
