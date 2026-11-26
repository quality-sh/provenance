//! The manifest field `disposition_actor_ids` as a person writes it: init does
//! not replace `agent` or `service` entries without a word, and an entry in
//! the text form is refused with the object form that an entry takes.

use crate::cli_review_decision_identity::{actor, Review, LEAD};
use crate::review_host_support::surfaces::Surface;
use crate::support::state_files::state_files;

use serde_json::json;

/// The complete error output of an init command with `--disposition-actor-id`
/// when the field holds the entries `agent ci_bot` and `service deployer`.
const INIT_REFUSAL: &str = "Error: The manifest field disposition_actor_ids has entries with the \
    identity type agent or service: agent ci_bot, service deployer. The flag \
    --disposition-actor-id writes only entries with the identity type human and replaces the \
    list, thus it would remove these entries. To change the list, edit the field in the file \
    .provenance/state/manifest.json.\n";

/// The message of the refusal of a manifest whose field holds the text entry
/// `lead_reviewer`.
const ENTRY_REFUSAL: &str =
    "the manifest field disposition_actor_ids has the text \"lead_reviewer\" \
    as an entry. Each entry is an object with identity_type and id, for example \
    {\"identity_type\":\"human\",\"id\":\"lead_reviewer\"}";

fn init(review: &Review, args: &[&str]) -> std::process::Output {
    let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.env(
        "PROVENANCE_TEST_STE100_ASSET_URL",
        env!("PROVENANCE_TEST_STE100_ASSET_URL"),
    );
    command.args(["init", "--path", review.root().to_str().unwrap()]);
    command.args(args).output().unwrap()
}

#[test]
fn init_refuses_to_replace_agent_and_service_entries_and_names_them() {
    let review = Review::new(&[LEAD], None);
    review.set_entries(json!([
        actor("human", LEAD),
        actor("agent", "ci_bot"),
        actor("service", "deployer")
    ]));
    let before = state_files(review.root());
    let refused = init(&review, &["--disposition-actor-id", "another_person"]);
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&refused.stderr), INIT_REFUSAL);
    assert_eq!(state_files(review.root()), before);
}

#[test]
fn an_entry_in_the_text_form_is_refused_with_the_object_form() {
    let review = Review::new(&[LEAD], None);
    review.set_entries(json!([LEAD]));
    let read = Surface::Cli.call(review.root(), "get", "requirements/req_review", None);
    assert_eq!(read.code, json!(1), "{}", read.text);
    assert_eq!(read.body["kind"], "invalid_state_file");
    assert_eq!(read.body["file"], "manifest.json");
    assert_eq!(read.body["line"], 3);
    assert_eq!(read.body["message"], ENTRY_REFUSAL);
}
