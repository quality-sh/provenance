//! An init with no PDF over a dictionary reference that does not parse
//! refuses before it downloads the dictionary and before each write, as
//! `rule_ste_dictionary_reference_parse_refusal` states. The tests of an init
//! with a PDF are in `cli_dictionary`.

use super::dictionary_support::{
    dictionary_pdf, file_names, half_reference, reference_path, reference_refusal,
    HALF_REFERENCE_MESSAGE,
};
use super::support::{serial, Fixture, TestServer};
use provenance_macros::verifies;

/// An initialized repository whose reference is half of a reference, with an
/// empty index directory.
fn damaged() -> Fixture {
    let fixture = Fixture::new();
    fixture.init().assert().success();
    std::fs::write(reference_path(&fixture.repo), half_reference()).unwrap();
    let _ = std::fs::remove_dir_all(&fixture.index_dir);
    std::fs::create_dir_all(&fixture.index_dir).unwrap();
    fixture
}

/// Runs init with no PDF and the asset URL `url`. Asserts the refusal, the
/// unchanged reference, no request to `server`, no new file in the asset
/// cache and no index file.
fn assert_refused(fixture: &Fixture, server: &TestServer, url: &str) {
    let assets = file_names(&fixture.asset_dir);

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
            file_names(&fixture.index_dir),
        ),
        (
            Some(1),
            reference_refusal(HALF_REFERENCE_MESSAGE),
            half_reference(),
            Vec::<String>::new(),
        )
    );
    assert_eq!(server.requests(), Vec::<String>::new());
    assert_eq!(file_names(&fixture.asset_dir), assets);
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
