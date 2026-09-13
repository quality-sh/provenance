use super::*;

#[cfg(unix)]
#[test]
fn missing_promised_blob_never_starts_a_remote_helper() {
    if !no_lazy_fetch_supported() {
        eprintln!(
            "skipping: git lacks --no-lazy-fetch (2.47+), so it cannot block \
             implicit promisor fetches; CI enforces this property on git 2.47+"
        );
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "-q"]);
    std::fs::write(root.join("code.rs"), "fn source() {}\n").unwrap();
    git(&["add", "code.rs"]);
    git(&["commit", "-qm", "Source"]);
    let head = git(&["rev-parse", "HEAD"]);
    let blob = git(&["rev-parse", "HEAD:code.rs"]);
    std::fs::remove_file(root.join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
    let sentinel = root.join("NETWORK_HELPER_STARTED");
    let helper = root.join("helper.sh");
    std::fs::write(
        &helper,
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", sentinel.as_str()),
    )
    .unwrap();
    git(&["config", "remote.origin.url", &format!("ext::sh {helper}")]);
    git(&["config", "remote.origin.promisor", "true"]);
    git(&["config", "extensions.partialClone", "origin"]);
    git(&["config", "protocol.ext.allow", "always"]);
    assert!(revision_files(root, &head).is_err());
    assert!(
        !sentinel.exists(),
        "read attempted an implicit object fetch"
    );
    // Prove that this fixture would invoke the helper without the safe command seam.
    let output = Command::new("git")
        .args(["show", &format!("{head}:code.rs")])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(sentinel.exists(), "fixture must exercise lazy fetching");
}
