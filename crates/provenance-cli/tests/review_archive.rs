//! Archive identity is checked before any build input directory appears.

#[path = "review_bundle/archive.rs"]
mod archive;

use std::{fmt::Write as _, io::Write, path::Path, path::PathBuf};

use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};

/// One-entry gzip tar with a raw ustar header, so hostile entry names reach the
/// reader unchanged on every host.
fn archive(root: &Path, name: &str, link: bool) -> (PathBuf, String) {
    const BODY: &[u8] = b"<!doctype html>";
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(format!("{:011o}\0", BODY.len()).as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    header[148..156].copy_from_slice(b"        ");
    header[156] = if link { b'2' } else { b'0' };
    if link {
        header[157..169].copy_from_slice(b"/etc/passwd\0");
    }
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());

    let mut payload = header.to_vec();
    payload.extend_from_slice(if link { &[][..] } else { BODY });
    payload.resize(payload.len().div_ceil(512) * 512, 0);
    payload.extend_from_slice(&[0u8; 1024]);

    let path = root.join("bundle.tar.gz");
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = GzEncoder::new(file, flate2::Compression::default());
    encoder.write_all(&payload).unwrap();
    encoder.finish().unwrap();
    let digest = {
        let mut text = String::new();
        for byte in Sha256::digest(std::fs::read(&path).unwrap()) {
            write!(text, "{byte:02x}").expect("hex digit");
        }
        text
    };
    (path, digest)
}

#[test]
fn verifies_bytes_before_creating_destination() {
    let dir = tempfile::tempdir().unwrap();
    let (bundle, digest) = archive(dir.path(), "index.html", false);
    let output = dir.path().join("output");
    let error = archive::extract_verified(&bundle, &output, &"0".repeat(64)).unwrap_err();
    assert!(error.to_string().contains("checksum"), "{error}");
    assert!(!output.exists());
    archive::extract_verified(&bundle, &output, &digest).unwrap();
    assert_eq!(
        std::fs::read(output.join("index.html")).unwrap(),
        b"<!doctype html>"
    );
}

#[test]
fn refuses_escape_links_and_existing_output() {
    let dir = tempfile::tempdir().unwrap();
    for (name, link) in [
        ("../escape", false),
        ("/escape", false),
        ("C:/escape", false),
        ("index.html", true),
    ] {
        let (bundle, digest) = archive(dir.path(), name, link);
        let output = dir.path().join("output");
        let error = archive::extract_verified(&bundle, &output, &digest).unwrap_err();
        assert!(
            error.to_string().contains("unsafe entry"),
            "{name}: {error}"
        );
        assert!(!output.exists(), "{name}");
    }
    let (bundle, digest) = archive(dir.path(), "index.html", false);
    let output = dir.path().join("existing");
    std::fs::create_dir(&output).unwrap();
    let error = archive::extract_verified(&bundle, &output, &digest).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
}
