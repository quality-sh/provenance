//! A document read on the HTTP route whose catch-up fails gives the cause:
//! the file, the line, the parse message and the steps that correct the state
//! for a state line that does not parse, and the text of a cause that has no
//! file, with no hint. A read with no stored revision refuses with
//! `no_projection` first, and gives the cause in the same members. A link
//! where a record file must be refuses the read before its catch-up, with
//! `read_failed` and a cause with no absolute path.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::depends_on_cycle::add_a_depends_on_cycle;
use support::records::Repository;
use support::resource_http::{call, host};
use support::state_lines::give_refines_a_string;

const DOCUMENT: &str = "/requirements/req_shared/document";

/// The hint of a read with no stored revision that `provenance materialize`
/// corrects.
const MATERIALIZE: &str = "run `provenance materialize`";

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
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
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
            "message": "invalid type: string \"req_parent\", expected a sequence",
            "hint": STATE_CORRECTIONS
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

/// A link where a record file must be stops the check that the manifest
/// declares the scope of the read, with an error that names the link by its
/// absolute path. The read refuses with `read_failed`, and its cause gives that
/// path relative to the repository.
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

    assert_eq!(
        (status, refused),
        (
            500,
            json!({"error": {"kind": "read_failed",
                "cause": format!("unsupported state entry: {shown}")}, "meta": {}})
        )
    );
}

/// The refusal of a read with no stored revision whose catch-up failed with
/// the cause `cause`, in the members of `document_catch_up_failed`.
fn no_projection(cause: &Value) -> Value {
    let mut error = cause.as_object().unwrap().clone();
    error.insert("kind".to_owned(), json!("no_projection"));
    json!({"error": error, "meta": {}})
}

/// A repository with no read database.
fn no_stored_revision(repo: &Repository) {
    // The writes of the fixture leave a read database; the test removes it.
    let cache = repo.dir.path().join(".provenance/cache");
    std::fs::remove_dir_all(&cache).unwrap();
}

/// With no stored revision, a document read whose catch-up stops at a state
/// line refuses with `no_projection`, as every other read does. The refusal
/// gives the line as `document_catch_up_failed` gives it, and the steps that
/// correct the state as the hint, not `provenance materialize`, which refuses
/// with the same line.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_document_read_with_no_stored_revision_refuses_with_no_projection() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    give_refines_a_string(repo.dir.path(), "req_shared");
    no_stored_revision(&repo);

    let (status, refused) = call(&host, "GET", DOCUMENT, None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        no_projection(&json!({
            "file": "scopes/default/requirements/req.jsonl",
            "line": 1,
            "message": "invalid type: string \"req_parent\", expected a sequence",
            "hint": STATE_CORRECTIONS
        }))
    );
}

/// With no stored revision, a record read whose catch-up stops at a cycle
/// refuses with `no_projection`. The cycle has no file and no correction of
/// its own, so the refusal gives only the message, which names the records.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_record_read_with_no_stored_revision_gives_the_fault_with_no_file() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    add_a_depends_on_cycle(repo.dir.path());
    no_stored_revision(&repo);

    let (status, refused) = call(&host, "GET", "/requirements/req_shared", None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        no_projection(&json!({"message": "depends_on forms a cycle: req_c -> req_d -> req_c"}))
    );
}

/// Sets the freshness policy of each read of `repo` to `policy`.
fn read_policy(repo: &Repository, policy: &str) {
    let path = repo.dir.path().join(".provenance/settings.json");
    let mut settings: Value = std::fs::read(&path).map_or_else(
        |_| json!({}),
        |bytes| serde_json::from_slice(&bytes).unwrap(),
    );
    settings["read"] = json!({ "freshness_policy": policy });
    std::fs::write(&path, settings.to_string()).unwrap();
}

/// An `annotate_only` read whose read database does not open refuses with
/// `no_projection`. The error of the database is the message, and
/// `provenance materialize`, which builds the database, is the hint.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_read_whose_database_does_not_open_gives_its_error_and_materialize() {
    let repo = Repository::new("The shared graph is readable.");
    read_policy(&repo, "annotate_only");
    no_stored_revision(&repo);
    let host = host(&repo, false);

    let (status, refused) = call(&host, "GET", "/requirements/req_shared", None).await;

    assert_eq!(status, 409);
    let unopened = "(code: 14) unable to open database file";
    assert_eq!(
        refused,
        no_projection(&json!({
            "message": format!("error returned from database: {unopened}: {unopened}"),
            "hint": MATERIALIZE
        }))
    );
}

/// An `annotate_only` read whose read database file exists and does not open
/// refuses with `no_projection` and the error of the database. The refusal has
/// no hint: `provenance materialize` refuses with the same error.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_read_whose_database_file_is_not_a_database_gives_no_hint() {
    let repo = Repository::new("The shared graph is readable.");
    read_policy(&repo, "annotate_only");
    no_stored_revision(&repo);
    let cache = repo.dir.path().join(".provenance/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("provenance.db"), [0x55; 4096]).unwrap();
    let host = host(&repo, false);

    let (status, refused) = call(&host, "GET", "/requirements/req_shared", None).await;

    assert_eq!(status, 409);
    let damaged = "(code: 26) file is not a database";
    assert_eq!(
        refused,
        no_projection(&json!({
            "message": format!("error returned from database: {damaged}: {damaged}")
        }))
    );
}

/// With no stored revision, a read whose catch-up stops at a record file
/// that is not UTF-8 refuses with `no_projection`. The native error names the
/// file by its absolute path; the message gives it relative to the
/// repository. No Rule states the paths in the cause of this refusal.
#[tokio::test]
async fn a_read_with_no_stored_revision_gives_no_absolute_path_in_the_cause() {
    let repo = Repository::new("The shared graph is readable.");
    let shown = ".provenance/state/scopes/default/requirements/req.jsonl";
    let mut bytes = std::fs::read(repo.dir.path().join(shown)).unwrap();
    bytes.extend_from_slice(b"\xff\n");
    std::fs::write(repo.dir.path().join(shown), bytes).unwrap();
    no_stored_revision(&repo);
    let host = host(&repo, false);

    let (status, refused) = call(&host, "GET", "/requirements/req_shared", None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        no_projection(&json!({
            "message": format!("read {shown}: the file is not UTF-8: invalid data")
        }))
    );
}

/// Removes the stored revision from the read database of `repo`, and keeps
/// each table, so that the database opens and holds no revision.
async fn remove_the_revision(repo: &Repository) {
    use sqlx::Connection as _;
    let database = repo.dir.path().join(".provenance/cache/provenance.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(database);
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    sqlx::query("DELETE FROM projection_revision")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
}

/// An `annotate_only` read of a read database that opens and holds no
/// revision has no cause. The refusal has only the hint, which names
/// `provenance materialize`.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_read_with_no_revision_and_no_cause_names_materialize() {
    let repo = Repository::new("The shared graph is readable.");
    let (status, read) = call(&host(&repo, false), "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    remove_the_revision(&repo).await;
    read_policy(&repo, "annotate_only");
    let host = host(&repo, false);

    let (status, refused) = call(&host, "GET", "/requirements/req_shared", None).await;

    assert_eq!(
        (status, refused),
        (409, no_projection(&json!({"hint": MATERIALIZE})))
    );
}
