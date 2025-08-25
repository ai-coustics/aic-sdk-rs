use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

pub struct Downloader {
    base_url: String,
    output_path: PathBuf,
    artifact_sha: HashMap<String, String>,
}

impl Downloader {
    pub fn new() -> Self {
        let version = env::var("CARGO_PKG_VERSION").unwrap();
        let base_url = "https://github.com/ai-coustics/aic-sdk-c/releases/download".to_string();

        let artifact_sha = HashMap::from([
            (
                format!("aic-sdk-aarch64-apple-darwin-{version}.tar.gz"),
                "35384d0e51733f39276c427a92f13d4a983404634604ec5fbeda10a3debc2860".to_string(),
            ),
            (
                format!("aic-sdk-aarch64-unknown-linux-gnu-{version}.tar.gz"),
                "3c10d6af456d8d6641f7e0f82e85145f79d7b1b6459c820e489f685296fafc28".to_string(),
            ),
            (
                format!("aic-sdk-x86_64-apple-darwin-{version}.tar.gz"),
                "a1e8050c8b87b645c2acb5bce396aa964640074b183da54248c2ef9549c41b6b".to_string(),
            ),
            (
                format!("aic-sdk-x86_64-pc-windows-msvc-{version}.zip"),
                "c6a414e23285e3c2930cae4c942f02aea30175a2986a2871304e6229b83bc91b".to_string(),
            ),
            (
                format!("aic-sdk-x86_64-unknown-linux-gnu-{version}.tar.gz"),
                "e22593f5cc6241be3d495d4a154c1157f298213e614cbe248a419745fc02e681".to_string(),
            ),
        ]);

        // Use a stable cache directory in target/ that doesn't change with OUT_DIR hash
        let cache_path = Self::get_cache_directory();

        Downloader {
            base_url,
            output_path: cache_path,
            artifact_sha,
        }
    }

    pub fn download(&self) -> PathBuf {
        let version = env::var("CARGO_PKG_VERSION").unwrap();
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
        let extracted_path = self.output_path.join(&file_prefix);

        // Check if library is already downloaded and extracted
        if self.is_library_valid(&extracted_path) {
            return extracted_path;
        }

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

        // Remove existing directory if it exists but is invalid
        if extracted_path.exists() {
            std::fs::remove_dir_all(&extracted_path)
                .expect("Failed to remove existing invalid library directory");
        }

        if file_extension == "zip" {
            extract_zip(&downloaded_file, &extracted_path);
        } else {
            extract_tgz(&downloaded_file, &extracted_path);
        }

        extracted_path
    }

    /// Check if the library has already been downloaded and extracted properly
    fn is_library_valid(&self, extracted_path: &Path) -> bool {
        // Check if the extracted directory exists
        if !extracted_path.exists() {
            return false;
        }

        // Check if the lib directory exists (required by the build script)
        let lib_dir = extracted_path.join("lib");
        if !lib_dir.exists() {
            return false;
        }

        // Check if the static library file exists
        let static_lib_ext = if cfg!(target_os = "windows") {
            ".lib"
        } else {
            ".a"
        };

        let static_lib = if cfg!(target_os = "windows") {
            lib_dir.join(format!("aic{}", static_lib_ext))
        } else {
            lib_dir.join(format!("libaic{}", static_lib_ext))
        };

        if !static_lib.exists() {
            return false;
        }

        // Check if the include directory exists (contains headers)
        let include_dir = extracted_path.join("include");
        if !include_dir.exists() {
            return false;
        }

        // Check if aic.h header file exists
        let header_file = include_dir.join("aic.h");
        if !header_file.exists() {
            return false;
        }

        true
    }

    /// Get a stable cache directory that persists across builds
    fn get_cache_directory() -> PathBuf {
        // Use OUT_DIR and navigate up to the target directory for a stable cache location
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        // OUT_DIR is typically: target/debug/build/{crate}-{hash}/out
        // Go up 4 levels to get to target/
        let target_dir = out_dir
            .ancestors()
            .nth(4)
            .expect("Could not find target directory from OUT_DIR");

        // Include version in cache directory name to avoid conflicts between different versions
        let version = env::var("CARGO_PKG_VERSION").unwrap();
        let version = version.replace('.', "_");
        let cache_dir = target_dir.join(format!("__aic_sdk_c_library_download_{}", version));

        // Create the cache directory if it doesn't exist
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");
        }

        cache_dir
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
