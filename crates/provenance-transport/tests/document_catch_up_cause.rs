//! A document read on the HTTP route whose catch-up fails gives the cause:
//! the file, the line and the parse message of a state line that does not
//! parse, and the text of a cause that has no file. A read with no stored
//! revision refuses with `no_projection` first.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::depends_on_cycle::add_a_depends_on_cycle;
use support::records::Repository;
use support::resource_http::{call, host};
use support::state_lines::give_refines_a_string;

const DOCUMENT: &str = "/requirements/req_shared/document";

/// A repository whose document was read one time, so that the projection
/// holds a revision.
async fn read_once() -> (Repository, provenance_transport::StatementHost) {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (status, first) = call(&host, "GET", DOCUMENT, None).await;
    assert_eq!(status, 200, "{first}");
    (repo, host)
}

/// The refusal of a document read whose catch-up failed with `cause`.
fn refusal(cause: &Value) -> Value {
    let mut error = cause.as_object().unwrap().clone();
    error.insert("kind".to_owned(), json!("document_catch_up_failed"));
    json!({"error": error, "meta": {}})
}

#[tokio::test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
async fn a_document_read_gives_the_line_that_stops_its_catch_up() {
    let (repo, host) = read_once().await;
    give_refines_a_string(repo.dir.path(), "req_shared");

    let (status, refused) = call(&host, "GET", DOCUMENT, None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        refusal(&json!({
            "file": "scopes/default/requirements/req.jsonl",
            "line": 1,
            "message": "invalid type: string \"req_parent\", expected a sequence"
        }))
    );
}

/// A cycle in the stored records has no file and no line: the document read
/// gives no file and no line, and the cycle as the message.
#[tokio::test]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
async fn a_document_read_gives_the_fault_with_no_file_that_stops_its_catch_up() {
    let (repo, host) = read_once().await;
    add_a_depends_on_cycle(repo.dir.path());

    let (status, refused) = call(&host, "GET", DOCUMENT, None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        refusal(&json!({"message": "depends_on forms a cycle: req_c -> req_d -> req_c"}))
    );
}

/// A link where a record file must be stops the catch-up with an error that
/// names the link by its absolute path. The refusal gives that path relative
/// to the repository.
#[cfg(unix)]
#[tokio::test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
async fn a_document_read_gives_no_absolute_path_in_the_cause() {
    let (repo, host) = read_once().await;
    let shown = ".provenance/state/scopes/default/requirements/req.jsonl";
    let file = repo.dir.path().join(shown);
    let copy = repo.dir.path().join("req.jsonl");
    std::fs::rename(&file, &copy).unwrap();
    std::os::unix::fs::symlink(&copy, &file).unwrap();

    let (status, refused) = call(&host, "GET", DOCUMENT, None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        refusal(&json!({"message": format!("unsupported state entry: {shown}")}))
    );
}

/// With no stored revision, a document read whose catch-up fails refuses
/// with `no_projection`, as every other read does.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_document_read_with_no_stored_revision_refuses_with_no_projection() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    give_refines_a_string(repo.dir.path(), "req_shared");
    let cache = repo.dir.path().join(".provenance/cache");
    if cache.exists() {
        std::fs::remove_dir_all(&cache).unwrap();
    }

    let (status, refused) = call(&host, "GET", DOCUMENT, None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        json!({"error": {"kind": "no_projection"}, "meta": {}})
    );
}
