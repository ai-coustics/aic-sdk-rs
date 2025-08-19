use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct Downloader {
    base_url: String,
    output_path: PathBuf,
    artifact_sha: HashMap<String, String>,
}

impl Downloader {
    pub fn new(output_path: &Path) -> Self {
        let base_url = format!(
            "https://github.com/ai-coustics/aic-sdk-c/releases/download",
        );

        let artifact_sha = HashMap::from([
            ("aic-sdk-aarch64-apple-darwin-0.6.2.tar.gz".to_string(),
            "7f2a3f1705113a8e0eb4e01cd24afbc959b84451b33fc9aaaea7475e71971fc6".to_string()),
            ("aic-sdk-aarch64-unknown-linux-gnu-0.6.2.tar.gz".to_string(),
            "25b065eb82dfd21934c244e0e1307bfa6b49af25c3e68986e8e1a0e6f151332d".to_string()),
            ("aic-sdk-x86_64-apple-darwin-0.6.2.tar.gz".to_string(),
            "5b86b2359427adb56000a88308c75cc9b636890abd5211120a5735ea1e75c696".to_string()),
            ("aic-sdk-x86_64-pc-windows-msvc-0.6.2.zip".to_string(),
            "c662138cd4d997fec0b1e0fd7312c1693d976a3fe15d4236409d1b645b879281".to_string()),
            ("aic-sdk-x86_64-unknown-linux-gnu-0.6.2.tar.gz".to_string(),
            "bc410a5aea213bcbf3a3dd9e2192fbd308ff2e15312479f559138814352cc15a".to_string()),
        ]);

        Downloader {
            base_url,
            output_path: output_path.to_path_buf(),
            artifact_sha,
        }
    }

    pub fn download(&self, version: &str) -> PathBuf {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let vendor = std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap();
        let abi = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();
        let os = match std::env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
            "macos" => "darwin".to_string(),
            os => os.to_string(),
        };

        let file_extension = if os == "windows" {
            "zip"
        } else {
            "tar.gz"
        };

        let triplet = format!("{arch}-{vendor}-{os}-{abi}");
        let file_prefix = format!("aic-sdk-{triplet}-{version}");
        let file_name = format!("{file_prefix}.{file_extension}");

        let expected_hash = self.artifact_sha.get(&file_name).expect(&format!("Invalid artifact name {}", file_name));
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
            unimplemented!("ZIP extraction not implemented yet");
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
