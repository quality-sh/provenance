use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use std::{net::TcpStream, thread, time::Duration};

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

#[path = "cli_ste_onboarding/support.rs"]
#[allow(dead_code)]
mod support;

use support::{init_args, serial, Fixture, TestServer};

#[test]
#[verifies("rule_ste_dictionary_agent_acquisition", examples)]
#[verifies("rule_ste_dictionary_download_identity", examples)]
fn onboarding_downloads_and_imports_the_official_asset() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();

    let output = fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "init failed: {stderr}");

    assert!(dictionary_support::reference_path(&fixture.repo).is_file());
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(!stdout.contains("Imported the Issue 9 dictionary"));
    assert!(
        stdout.contains("  .provenance/state/dictionary.json (added the dictionary reference)\n")
    );
    assert!(stdout.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    let requests = server.requests();
    assert_eq!(
        requests.len(),
        1,
        "recorded requests: {requests:#?}\ncli stderr: {stderr}"
    );
    assert!(requests[0].to_ascii_lowercase().contains(&format!(
        "user-agent: provenance/{}",
        env!("CARGO_PKG_VERSION")
    )));
}

#[test]
#[verifies("rule_ste_dictionary_import_identity", examples)]
fn onboarding_imports_a_selected_pdf_without_network_access() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();
    let pdf = fixture.temporary.path().join("issue-9.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();

    fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .args(["--ste-pdf", pdf.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported the Issue 9 dictionary").not())
        .stdout(predicate::str::contains(
            "  .provenance/state/dictionary.json (added the dictionary reference)\n",
        ));

    assert!(dictionary_support::reference_path(&fixture.repo).is_file());
    assert_eq!(
        server.requests().len(),
        0,
        "a selected PDF must not touch the network"
    );
}

#[test]
fn onboarding_accepts_a_relative_pdf_path() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();
    std::fs::write(
        fixture.temporary.path().join("issue-9.pdf"),
        dictionary_support::dictionary_pdf(),
    )
    .unwrap();

    fixture
        .command()
        .current_dir(fixture.temporary.path())
        .args(init_args(&fixture.repo))
        .args(["--ste-pdf", "issue-9.pdf"])
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();

    assert!(dictionary_support::reference_path(&fixture.repo).is_file());
    assert_eq!(server.requests().len(), 0);
}

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
#[verifies("rule_ste_dictionary_no_operational_download", examples)]
fn repeated_setup_and_normal_checks_use_local_data_without_network_access() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();
    let url = server.url();
    fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", &url)
        .assert()
        .success();

    fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", &url)
        .assert()
        .success();
    fixture
        .command()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", &url)
        .args(["check", "--repo", fixture.repo.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(server.requests().len(), 1);
}

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn new_projects_reuse_a_verified_index_and_reject_a_changed_asset() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_support::dictionary_pdf());
    let fixture = Fixture::new();
    fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();
    let another_repo = fixture.temporary.path().join("another-repo");

    fixture
        .command()
        .args(init_args(&another_repo))
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();

    assert_eq!(server.requests().len(), 1);
    assert_eq!(
        std::fs::read(dictionary_support::reference_path(&fixture.repo)).unwrap(),
        std::fs::read(dictionary_support::reference_path(&another_repo)).unwrap()
    );

    let asset = fixture.asset_dir.join("ASD-STE100_ISSUE9.pdf");
    std::fs::write(&asset, b"not a PDF").unwrap();
    let third_repo = fixture.temporary.path().join("third-repo");
    fixture
        .command()
        .args(init_args(&third_repo))
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .assert()
        .success();
    assert_eq!(server.requests().len(), 2);
    assert!(dictionary_support::reference_path(&third_repo).is_file());
    assert_eq!(
        std::fs::read(asset).unwrap(),
        dictionary_support::dictionary_pdf()
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_digest_verification", examples)]
fn invalid_shared_indexes_are_rebuilt_from_the_cached_pdf() {
    let _serial = serial();
    for corruption in ["truncated", "digest", "identity"] {
        let server = TestServer::new(200, dictionary_support::dictionary_pdf());
        let fixture = Fixture::new();
        fixture
            .init()
            .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
            .assert()
            .success();
        let index = std::fs::read_dir(&fixture.index_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .expect("the first init stored an index");
        let mut stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
        match corruption {
            "digest" => stored["entries"][0]["headword"] = "CHANGED".into(),
            "identity" => stored["identity"]["source_sha256"] = "wrong".into(),
            _ => {}
        }
        let damaged = if corruption == "truncated" {
            b"{\"identity\":".to_vec()
        } else {
            serde_json::to_vec(&stored).unwrap()
        };
        std::fs::write(&index, damaged).unwrap();
        let another_repo = fixture.temporary.path().join("another-repo");

        fixture
            .command()
            .args(init_args(&another_repo))
            .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
            .assert()
            .success();

        assert_eq!(server.requests().len(), 1, "corruption: {corruption}");
        assert!(dictionary_support::reference_path(&another_repo).is_file());
        let restored: provenance_ste100::DictionaryImport =
            serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
        assert_eq!(&restored, dictionary_support::imported_dictionary());
    }
}

#[test]
#[verifies("rule_ste_dictionary_download_retry_bound", examples)]
#[verifies("rule_ste_dictionary_asset_fallback", examples)]
fn exhausted_download_retries_fall_back_to_a_loud_warning() {
    let _serial = serial();
    let server = TestServer::new(503, b"unavailable");
    let fixture = Fixture::new();

    let output = fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", server.url())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "init failed: {stdout}");
    assert!(stdout.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    assert!(!stdout.contains("Warning:"), "stdout: {stdout}");
    assert!(stderr.contains("Warning:"), "stderr: {stderr}");
    assert!(stderr.contains("rerun init"), "stderr: {stderr}");
    assert!(stderr.contains("dictionary import"), "stderr: {stderr}");
    assert!(
        !stderr.to_ascii_lowercase().contains("asd-ste100.org"),
        "the warning carries no ASD link: {stderr}"
    );
    assert!(!stdout.contains("ASD owns"), "stdout: {stdout}");

    assert_eq!(
        server.requests().len(),
        3,
        "recorded requests: {:#?}",
        server.requests()
    );
    assert!(!dictionary_support::reference_path(&fixture.repo).exists());
}

#[test]
#[verifies("rule_ste_dictionary_download_off", examples)]
fn the_off_asset_url_skips_the_download_and_falls_back_to_the_warning() {
    let _serial = serial();
    let fixture = Fixture::new();
    // A file in place of the asset cache directory: a download cannot start
    // without that directory, so this init cannot use the network.
    std::fs::write(&fixture.asset_dir, b"not a directory").unwrap();

    let output = fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", "off")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "init failed: {stderr}");
    assert!(stdout.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    assert!(
        stderr.contains("Warning: the official Issue 9 dictionary could not be added (the dictionary download is off)"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("rerun init"), "stderr: {stderr}");
    assert!(stderr.contains("dictionary import"), "stderr: {stderr}");
    assert!(!stderr.contains("asset cache"), "stderr: {stderr}");
    assert!(!dictionary_support::reference_path(&fixture.repo).exists());
}

#[test]
fn init_help_omits_the_removed_ste_onboarding_flag() {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--ste-pdf"))
        .stdout(predicate::str::contains("--ste-onboarding").not());
}

#[test]
fn server_ignores_connections_that_send_nothing() {
    let _serial = serial();
    let server = TestServer::new(200, b"body");
    // Same shape as Drop's wake-up socket: connect, send nothing, hang up.
    drop(TcpStream::connect(server.address).unwrap());
    thread::sleep(Duration::from_millis(50));
    assert_eq!(server.requests().len(), 0);
}
