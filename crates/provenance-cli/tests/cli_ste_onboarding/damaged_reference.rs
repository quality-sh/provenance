//! An init with no PDF over a dictionary reference that does not parse
//! refuses before it downloads the dictionary and before each write, as
//! `rule_ste_dictionary_reference_parse_refusal` states. The tests of an init
//! with a PDF are in `cli_dictionary`.

use super::dictionary_support::{dictionary_pdf, reference_path};
use super::support::{serial, Fixture, TestServer};
use provenance_macros::verifies;
use std::path::Path;

/// A reference that stops in the name of its third field. It holds a
/// technical name that a write must not lose.
const HALF: &str = r#"{"technical_names": ["bword"], "issue": 9, "source_sha"#;

/// The text of the refusal of [`HALF`].
const REFUSAL: &str = concat!(
    r#"Error: {"error":{"file":"dictionary.json","kind":"invalid_state_file","line":1,"#,
    r#""message":"EOF while parsing a string"},"meta":{}}; get the file again from git or "#,
    "correct its JSON, and then do the command again\n"
);

/// An initialized repository whose reference is [`HALF`].
fn damaged() -> Fixture {
    let fixture = Fixture::new();
    fixture.init().assert().success();
    std::fs::write(reference_path(&fixture.repo), HALF).unwrap();
    fixture
}

/// The names in the asset cache directory, sorted.
fn asset_files(directory: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Runs init with no PDF and the asset URL `url`. Asserts the refusal, the
/// unchanged reference, no request to `server` and no new file in the asset
/// cache.
fn assert_refused(fixture: &Fixture, server: &TestServer, url: &str) {
    let assets = asset_files(&fixture.asset_dir);

    let output = fixture
        .init()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", url)
        .output()
        .unwrap();

    assert_eq!(
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            std::fs::read_to_string(reference_path(&fixture.repo)).unwrap(),
        ),
        (Some(1), REFUSAL.to_owned(), HALF.to_owned())
    );
    assert_eq!(server.requests(), Vec::<String>::new());
    assert_eq!(asset_files(&fixture.asset_dir), assets);
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_with_the_download_off_refuses_half_of_a_reference() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_pdf());
    let fixture = damaged();

    // An `https` URL is not a loopback `http` URL, so the download is off.
    let off = server.url().replacen("http://", "https://", 1);
    assert_refused(&fixture, &server, &off);
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_with_an_asset_url_that_answers_refuses_half_of_a_reference() {
    let _serial = serial();
    let server = TestServer::new(200, dictionary_pdf());
    let fixture = damaged();

    assert_refused(&fixture, &server, &server.url());
}
