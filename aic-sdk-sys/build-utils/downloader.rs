use std::{
    collections::HashMap,
    fs,
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
        let base_url = "https://github.com/ai-coustics/aic-sdk-c/releases/download".to_string();

        let (version, artifact_sha) = read_checksums_from_file();

        // Validate that the current target platform exists in the checksum file
        validate_target_exists(&artifact_sha, &version);

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

fn read_checksums_from_file() -> (String, HashMap<String, String>) {
    let checksum_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("checksum.txt");
    let checksum_content = fs::read_to_string(&checksum_path).expect("Failed to read checksum.txt");

    let mut artifact_sha = HashMap::new();
    let mut version = None;

    for line in checksum_content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }

        let (hash, filename) = (parts[0], parts[1]);

        // Extract version from filename (e.g., "aic-sdk-x86_64-apple-darwin-0.11.0.tar.gz")
        if version.is_none()
            && let Some(v) = extract_version_from_filename(filename)
        {
            version = Some(v);
        }

        artifact_sha.insert(filename.to_string(), hash.to_string());
    }

    let version = version.expect("Could not determine version from checksum.txt");
    (version, artifact_sha)
}

fn validate_target_exists(artifact_sha: &HashMap<String, String>, version: &str) {
    let target = std::env::var("TARGET").expect("TARGET environment variable not set");

    // Check both .tar.gz and .zip extensions
    let file_name_tar = format!("aic-sdk-{}-{}.tar.gz", target, version);
    let file_name_zip = format!("aic-sdk-{}-{}.zip", target, version);

    if !artifact_sha.contains_key(&file_name_tar) && !artifact_sha.contains_key(&file_name_zip) {
        panic!(
            "Target platform '{}' (tried: {} and {}) not found in checksum.txt. Available platforms: {}",
            target,
            file_name_tar,
            file_name_zip,
            artifact_sha
                .keys()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn extract_version_from_filename(filename: &str) -> Option<String> {
    // Example: "aic-sdk-x86_64-apple-darwin-0.11.0.tar.gz" -> "0.11.0"
    // Example: "aic-sdk-aarch64-pc-windows-msvc-0.11.0.zip" -> "0.11.0"

    // Remove file extension
    let name = filename
        .strip_suffix(".tar.gz")
        .or_else(|| filename.strip_suffix(".zip"))?;

    // Split by '-' and find the version pattern (starts with digit)
    let parts: Vec<&str> = name.split('-').collect();

    // Version is typically the last part that starts with a digit
    for part in parts.iter().rev() {
        if part.chars().next()?.is_ascii_digit() {
            return Some(part.to_string());
        }
    }

    None
}

fn fetch_file(source_url: &str) -> Vec<u8> {
    ureq::get(source_url)
        .call()
        .unwrap()
        .body_mut()
        .with_config()
        .limit(400 * 1024 * 1024) // 400 MB
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
