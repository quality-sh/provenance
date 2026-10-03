#![cfg(target_os = "linux")]

use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read},
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(
        directory.path().to_str().unwrap(),
    );
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = provenance_core::Manifest::default_with_scope(
        provenance_core::ScopeId::new("default").unwrap(),
        provenance_core::RepoPathPrefix::new("."),
    );
    std::fs::write(layout.manifest_path(), serde_json::to_vec(&manifest).unwrap()).unwrap();
    directory
}

fn opener() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("xdg-open");
    std::fs::write(&executable, "#!/bin/sh\nprintf '%s' \"$1\" > \"$OPEN_RECORD\"\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let record = directory.path().join("opened-url");
    (directory, record)
}

fn failing_opener() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("xdg-open");
    std::fs::write(&executable, "#!/bin/sh\nexit 7\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

fn start(
    repository: &std::path::Path,
    opener_directory: &std::path::Path,
    record: &std::path::Path,
    no_open: bool,
    display: bool,
) -> (Child, Value) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    command.args([
        "review",
        "--repo",
        repository.to_str().unwrap(),
        "--repository-id",
        "local",
        "--scope",
        "default",
    ]);
    if no_open {
        command.arg("--no-open");
    }
    command
        .env("PATH", opener_directory)
        .env("OPEN_RECORD", record)
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if display {
        command.env("DISPLAY", ":1");
    } else {
        command.env_remove("DISPLAY");
    }
    let mut child = command.spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    (child, serde_json::from_str(&line).unwrap())
}

fn wait_for(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !path.exists() {
        assert!(Instant::now() < deadline, "browser opener was not invoked");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn create_requirement(repository: &std::path::Path) {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "req_open",
            "create",
            "--type",
            "requirement",
            "--repo",
            repository.to_str().unwrap(),
            "--statement",
            "The command opens the review link.",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn review_start_opens_its_launch_link() {
    let repository = repository();
    let (opener, record) = opener();
    let (mut child, startup) = start(repository.path(), opener.path(), &record, false, true);

    wait_for(&record);
    assert_eq!(std::fs::read_to_string(&record).unwrap(), startup["url"]);
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn no_open_skips_the_platform_opener() {
    let repository = repository();
    let (opener, record) = opener();
    let (mut child, _) = start(repository.path(), opener.path(), &record, true, true);

    std::thread::sleep(Duration::from_millis(100));
    assert!(!record.exists());
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn a_headless_session_prints_the_link_without_opening_it() {
    let repository = repository();
    let (opener, record) = opener();
    let (mut child, startup) = start(repository.path(), opener.path(), &record, false, false);

    std::thread::sleep(Duration::from_millis(100));
    assert!(!record.exists());
    let _ = child.kill();
    let _ = child.wait();
    let mut stderr = String::new();
    child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    assert!(stderr.contains(startup["url"].as_str().unwrap()), "{stderr}");
    assert!(stderr.contains("display"), "{stderr}");
}

#[test]
fn review_link_command_opens_unless_no_open_is_set() {
    let repository = repository();
    create_requirement(repository.path());
    let (opener, record) = opener();
    let (mut host, _) = start(repository.path(), opener.path(), &record, true, true);
    let command = || {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
        command
            .args([
                "req_open",
                "get",
                "--repo",
                repository.path().to_str().unwrap(),
                "--review-link",
            ])
            .env("PATH", opener.path())
            .env("OPEN_RECORD", &record)
            .env("DISPLAY", ":1")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_CLIENT")
            .env_remove("SSH_TTY")
            .env_remove("WAYLAND_DISPLAY");
        command
    };

    let output = command().output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    wait_for(&record);
    assert_eq!(
        std::fs::read_to_string(&record).unwrap(),
        String::from_utf8(output.stdout).unwrap().trim()
    );
    std::fs::remove_file(&record).unwrap();

    let output = command().arg("--no-open").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!record.exists());
    let _ = host.kill();
    let _ = host.wait();
}

#[test]
fn opener_failure_warns_without_failing_the_review_link_command() {
    let repository = repository();
    create_requirement(repository.path());
    let (host_opener, record) = opener();
    let (mut host, _) = start(repository.path(), host_opener.path(), &record, true, true);
    let opener = failing_opener();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "req_open",
            "get",
            "--repo",
            repository.path().to_str().unwrap(),
            "--review-link",
        ])
        .env("PATH", opener.path())
        .env("DISPLAY", ":1")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_ne!(output.stdout, Vec::<u8>::new());
    let warning = String::from_utf8(output.stderr).unwrap();
    assert_eq!(warning.lines().count(), 1, "{warning}");
    assert!(warning.contains("warning:"), "{warning}");
    let _ = host.kill();
    let _ = host.wait();
}
