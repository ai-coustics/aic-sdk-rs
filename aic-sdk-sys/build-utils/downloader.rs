use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

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
        let target = std::env::var("TARGET").unwrap();
        let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

        let file_name = artifact_file_name(&target, &os, version);
        let file_prefix = format!("aic-sdk-{target}-{version}");

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

        // Decide the archive format from the artifact name rather than the OS: Windows has two
        // flavours (MSVC ships `.zip`, GNU/LLVM `gnullvm` ships `.tar.gz`), so `os == "windows"`
        // alone is no longer enough.
        if file_name.ends_with(".zip") {
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
        if let Some(v) = extract_version_from_filename(filename) {
            if v != CRATE_VERSION {
                panic!(
                    "Checksum manifest version ({}) does not match aic-sdk-sys crate version ({}). Update checksum.txt to match the crate version.",
                    v, CRATE_VERSION
                );
            }

            if version.is_none() {
                version = Some(CRATE_VERSION.to_string());
            }
        }

        artifact_sha.insert(filename.to_string(), hash.to_string());
    }

    let version = version.unwrap_or_else(|| {
        panic!(
            "Could not determine version from checksum.txt. Expected aic-sdk version {}",
            CRATE_VERSION
        )
    });
    (version, artifact_sha)
}

fn validate_target_exists(artifact_sha: &HashMap<String, String>, version: &str) {
    let target = std::env::var("TARGET").expect("TARGET environment variable not set");

    // Check both .tar.gz and .zip extensions
    let file_name_tar = format!("aic-sdk-{}-{}.tar.gz", target, version);
    let file_name_zip = format!("aic-sdk-{}-{}.zip", target, version);

    if !artifact_sha.contains_key(&file_name_tar) && !artifact_sha.contains_key(&file_name_zip) {
        panic!(
            "Target platform not available in aic-sdk. Available platforms: {}",
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

fn artifact_file_name(target: &str, os: &str, version: &str) -> String {
    // Windows MSVC artifacts are distributed as `.zip`. Every other platform uses `.tar.gz`,
    // including the Windows GNU/LLVM (`*-pc-windows-gnullvm`) target, which ships GNU-style
    // files (`libaic.a`, `libaic.dll.a`, `aic.dll`) in a tarball like the Unix platforms.
    let ext = if os == "windows" && target.ends_with("msvc") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("aic-sdk-{target}-{version}.{ext}")
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

#[cfg(test)]
mod tests {
    use super::*;

    // All supported targets paired with their CARGO_CFG_TARGET_OS value.
    const TARGETS: &[(&str, &str)] = &[
        ("aarch64-linux-android", "android"),
        ("aarch64-unknown-linux-gnu", "linux"),
        ("x86_64-unknown-linux-gnu", "linux"),
        ("aarch64-apple-darwin", "macos"),
        ("x86_64-apple-darwin", "macos"),
        ("aarch64-apple-ios", "ios"),
        ("aarch64-apple-ios-macabi", "ios"),
        ("aarch64-apple-ios-sim", "ios"),
        ("aarch64-apple-tvos", "tvos"),
        ("aarch64-apple-tvos-sim", "tvos"),
        ("aarch64-apple-visionos", "visionos"),
        ("aarch64-apple-visionos-sim", "visionos"),
        ("x86_64-apple-ios-macabi", "ios"),
        ("armv7-linux-androideabi", "android"),
        ("x86_64-linux-android", "android"),
        ("aarch64-pc-windows-msvc", "windows"),
        ("x86_64-pc-windows-msvc", "windows"),
    ];

    // Targets whose upstream aic-sdk-c artifact has not been published yet. They are kept
    // separate from `TARGETS` so the checksum-presence test (`artifact_file_name_all_targets`)
    // does not require a `checksum.txt` entry that does not exist yet. Once a target's artifact
    // and checksum are published upstream, move its entry into `TARGETS` above.
    const PENDING_TARGETS: &[(&str, &str)] = &[("x86_64-pc-windows-gnullvm", "windows")];

    fn read_checksum_filenames() -> std::collections::HashSet<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("checksum.txt");
        let content = std::fs::read_to_string(&path).expect("Failed to read checksum.txt");
        content
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1).map(str::to_owned))
            .collect()
    }

    #[test]
    fn artifact_file_name_all_targets() {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        let checksums = read_checksum_filenames();

        let missing: Vec<&str> = TARGETS
            .iter()
            .filter(|&&(target, os)| !checksums.contains(&artifact_file_name(target, os, VERSION)))
            .map(|&(target, _)| target)
            .collect();

        assert!(
            missing.is_empty(),
            "checksum.txt is missing entries for: {}",
            missing.join(", "),
        );
    }

    #[test]
    fn non_windows_targets_use_tar_gz() {
        for &(target, os) in TARGETS {
            if os != "windows" {
                let name = artifact_file_name(target, os, "0.0.0");
                assert!(
                    name.ends_with(".tar.gz"),
                    "expected .tar.gz for target '{target}', got '{name}'",
                );
            }
        }
    }

    #[test]
    fn windows_targets_use_expected_extension() {
        // MSVC Windows targets ship `.zip`; GNU/LLVM (`gnullvm`) Windows targets ship `.tar.gz`.
        // Cover both the published and the pending targets so this stays correct once a pending
        // entry is promoted into `TARGETS`.
        for &(target, os) in TARGETS.iter().chain(PENDING_TARGETS) {
            if os != "windows" {
                continue;
            }
            let name = artifact_file_name(target, os, "0.0.0");
            let expected = if target.ends_with("msvc") {
                ".zip"
            } else {
                ".tar.gz"
            };
            assert!(
                name.ends_with(expected),
                "expected {expected} for target '{target}', got '{name}'",
            );
        }
    }

    #[test]
    fn pending_targets_have_no_checksum_yet() {
        // Forcing function: when an upstream artifact is finally published and its checksum is
        // added to checksum.txt, this guard fails on purpose. That is the reminder to move the
        // target out of PENDING_TARGETS and into TARGETS so it gets full coverage (including the
        // checksum-presence check in `artifact_file_name_all_targets`).
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        let checksums = read_checksum_filenames();

        for &(target, os) in PENDING_TARGETS {
            let name = artifact_file_name(target, os, VERSION);
            assert!(
                !checksums.contains(&name),
                "checksum for '{name}' now exists; move {target} from PENDING_TARGETS into TARGETS",
            );
        }
    }

    #[test]
    fn artifact_file_name_embeds_target_and_version() {
        let name = artifact_file_name("aarch64-linux-android", "android", "1.2.3");
        assert_eq!(name, "aic-sdk-aarch64-linux-android-1.2.3.tar.gz");

        let name = artifact_file_name("x86_64-pc-windows-msvc", "windows", "1.2.3");
        assert_eq!(name, "aic-sdk-x86_64-pc-windows-msvc-1.2.3.zip");

        // The GNU/LLVM Windows target shares the Windows OS but uses a `.tar.gz` like Unix.
        let name = artifact_file_name("x86_64-pc-windows-gnullvm", "windows", "1.2.3");
        assert_eq!(name, "aic-sdk-x86_64-pc-windows-gnullvm-1.2.3.tar.gz");

        let name = artifact_file_name("aarch64-apple-ios-macabi", "ios", "1.2.3");
        assert_eq!(name, "aic-sdk-aarch64-apple-ios-macabi-1.2.3.tar.gz");

        let name = artifact_file_name("aarch64-apple-tvos-sim", "tvos", "1.2.3");
        assert_eq!(name, "aic-sdk-aarch64-apple-tvos-sim-1.2.3.tar.gz");

        let name = artifact_file_name("aarch64-apple-visionos-sim", "visionos", "1.2.3");
        assert_eq!(name, "aic-sdk-aarch64-apple-visionos-sim-1.2.3.tar.gz");
    }

    #[test]
    fn extract_version_from_filename_tar_gz() {
        assert_eq!(
            extract_version_from_filename("aic-sdk-x86_64-apple-darwin-0.11.0.tar.gz"),
            Some("0.11.0".to_string()),
        );
    }

    #[test]
    fn extract_version_from_filename_zip() {
        assert_eq!(
            extract_version_from_filename("aic-sdk-aarch64-pc-windows-msvc-0.19.0.zip"),
            Some("0.19.0".to_string()),
        );
    }

    #[test]
    fn extract_version_from_filename_android() {
        assert_eq!(
            extract_version_from_filename("aic-sdk-aarch64-linux-android-0.19.0.tar.gz"),
            Some("0.19.0".to_string()),
        );
    }

    #[test]
    fn extract_version_from_filename_macabi() {
        assert_eq!(
            extract_version_from_filename("aic-sdk-aarch64-apple-ios-macabi-0.19.0.tar.gz"),
            Some("0.19.0".to_string()),
        );
    }

    #[test]
    fn extract_version_from_filename_invalid() {
        assert_eq!(extract_version_from_filename("invalid"), None);
        assert_eq!(extract_version_from_filename("no-version.tar.gz"), None);
    }
}
