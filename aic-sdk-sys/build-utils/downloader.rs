use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct Downloader {
    base_url: String,
    version: String,
    output_path: PathBuf,
    artifact_sha: HashMap<String, String>,
}

impl Downloader {
    pub fn new(output_path: &Path) -> Self {
        let version = "0.10.0".to_string();
        let base_url = "https://github.com/ai-coustics/aic-sdk-c/releases/download".to_string();

        let artifact_sha = HashMap::from([
            (
                format!("aic-sdk-aarch64-apple-darwin-{version}.tar.gz"),
                "e677d07de9c7558115a695ca8e9a315db1d19093c191bbd4f57eaf3fa9ad9e30".to_string(),
            ),
            (
                format!("aic-sdk-aarch64-pc-windows-msvc-{version}.zip"),
                "c07d019646f5979ea291f8a6df7f4797ceafa3e6efac8b1c82229619d72599da".to_string(),
            ),
            (
                format!("aic-sdk-aarch64-unknown-linux-gnu-{version}.tar.gz"),
                "a1cd0afc8a8c7e2cefcd10f6c57fd859bdf7a3d037149c36658df287c8946af1".to_string(),
            ),
            (
                format!("aic-sdk-x86_64-apple-darwin-{version}.tar.gz"),
                "f785b0fabe955b7645903e81049062832c6b035f916d5e06584fb85ac1688a9b".to_string(),
            ),
            (
                format!("aic-sdk-x86_64-pc-windows-msvc-{version}.zip"),
                "f5aa1e1513276b663406b2593f958c30263a6d31220a29a36ae11707b449da94".to_string(),
            ),
            (
                format!("aic-sdk-x86_64-unknown-linux-gnu-{version}.tar.gz"),
                "e046098dd67b10516033b2a7da5c78703bb604c5034c9df371b0fc9fe800597b".to_string(),
            ),
        ]);

        Downloader {
            base_url,
            version,
            output_path: output_path.to_path_buf(),
            artifact_sha,
        }
    }

    pub fn download(&self) -> PathBuf {
        let version = self.version.as_str();
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let vendor = std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap();
        let abi = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();
        let os = match std::env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
            "macos" => "darwin".to_string(),
            os => os.to_string(),
        };

        let triplet = format!("{arch}-{vendor}-{os}");

        let file_extension = if os == "windows" { "zip" } else { "tar.gz" };
        let file_prefix = if vendor == "apple" {
            format!("aic-sdk-{triplet}-{version}")
        } else {
            format!("aic-sdk-{triplet}-{abi}-{version}")
        };

        let file_name = format!("{file_prefix}.{file_extension}");

        let expected_hash = self
            .artifact_sha
            .get(&file_name)
            .unwrap_or_else(|| panic!("Invalid artifact name {}", file_name));
        let url = format!("{}/{}/{}", self.base_url, version, file_name);

        let downloaded_file = fetch_file(&url);
        let downloaded_hash = sha256(&downloaded_file);

        assert_eq!(
            &downloaded_hash, expected_hash,
            "SHA mismatch: {} != {}",
            &downloaded_hash, expected_hash
        );

        let extracted_path = self.output_path.join(&file_prefix);

        if file_extension == "zip" {
            extract_zip(&downloaded_file, &extracted_path);
        } else {
            extract_tgz(&downloaded_file, &extracted_path);
        }

        extracted_path
    }
}

fn fetch_file(source_url: &str) -> Vec<u8> {
    ureq::get(source_url)
        .call()
        .unwrap()
        .body_mut()
        .with_config()
        .limit(300 * 1024 * 1024) // 300 MB
        .read_to_vec()
        .unwrap()
}

fn bytes_to_hex_str(bytes: Vec<u8>) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

fn sha256(buf: &[u8]) -> String {
    let hash_bytes: Vec<u8> = <sha2::Sha256 as sha2::Digest>::digest(buf).to_vec();
    bytes_to_hex_str(hash_bytes)
}

fn extract_tgz(buf: &[u8], output: &Path) {
    let buf: std::io::BufReader<&[u8]> = std::io::BufReader::new(buf);
    let tar = flate2::read::GzDecoder::new(buf);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(output).expect("Failed to extract .tgz file");
}

fn extract_zip(buf: &[u8], output: &Path) {
    let cursor = std::io::Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(cursor).expect("Failed to read ZIP archive");

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .expect("Failed to get file from ZIP archive");
        let file_path = output.join(file.name());

        if file.is_dir() {
            // This is a directory
            std::fs::create_dir_all(&file_path).expect("Failed to create directory");
        } else {
            // This is a file
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create parent directory");
            }

            let mut output_file =
                std::fs::File::create(&file_path).expect("Failed to create output file");
            std::io::copy(&mut file, &mut output_file).expect("Failed to extract file");

            // Set file permissions on Unix systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let permissions = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&file_path, permissions)
                        .expect("Failed to set file permissions");
                }
            }
        }
    }
}
