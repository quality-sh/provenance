use super::ReadError;
use crate::cache::completion::Stage;
use crate::cache::CompletionTimedOut;
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::{MovedUnit as NativeMovedUnit, ReadRefusal};
use crate::stale::git::GitRefusal;
use camino::Utf8PathBuf;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_core::protocol::read_failure::{CacheCloseStage, MovedUnit, ReadFailure};
use provenance_macros::verifies;
use std::time::Duration;

fn read_error(error: impl std::error::Error + Send + Sync + 'static) -> ReadError {
    ReadError(anyhow::Error::new(error))
}

fn database() -> Utf8PathBuf {
    Utf8PathBuf::from(".provenance/cache.sqlite")
}

#[test]
fn a_native_refusal_publishes_its_closed_failure_and_status() {
    let cases = [
        (
            read_error(ReadFailure::ResourceNotFound),
            ReadFailure::ResourceNotFound,
            404,
        ),
        (
            read_error(GitRefusal::Unavailable {
                source: anyhow::anyhow!("git is not installed"),
            }),
            ReadFailure::GitUnavailable,
            503,
        ),
        (
            read_error(GitRefusal::RevisionNotFound {
                diagnostic: "unknown revision".into(),
            }),
            ReadFailure::GitRevisionNotFound,
            409,
        ),
        (
            read_error(FileAccessRefusal::Unavailable),
            ReadFailure::FileUnavailable,
            503,
        ),
        (
            read_error(FileAccessRefusal::Denied),
            ReadFailure::FileAccessDenied,
            403,
        ),
        (
            read_error(FileAccessRefusal::Missing),
            ReadFailure::ReadFailed {
                cause: "repository file not found".into(),
            },
            500,
        ),
        (
            read_error(ReadRefusal::NoProjection {
                database: database(),
                because: String::new(),
            }),
            ReadFailure::NoProjection,
            409,
        ),
        (
            read_error(ReadRefusal::SchemaBehind {
                database: database(),
            }),
            ReadFailure::SchemaBehind,
            409,
        ),
        (
            read_error(ReadRefusal::HalfMigrated {
                database: database(),
            }),
            ReadFailure::HalfMigrated,
            409,
        ),
        (
            ReadError(anyhow::anyhow!("an error of no known kind")),
            ReadFailure::ReadFailed {
                cause: "an error of no known kind".into(),
            },
            500,
        ),
    ];
    for (error, failure, status) in cases {
        assert_eq!(error.safe(), failure, "{error}");
        assert_eq!(error.status(), status, "{error}");
    }
}

#[test]
fn a_stale_refusal_keeps_the_revision_facts_and_drops_the_local_paths() {
    let error = read_error(ReadRefusal::Stale {
        database: database(),
        serial: 7,
        digest: "digest".into(),
        instance_id: "instance".into(),
        moved: vec![NativeMovedUnit {
            unit: "requirements".into(),
            stored: "old".into(),
            live: "new".into(),
        }],
    });
    let expected = ReadFailure::Stale {
        serial: 7,
        digest: "digest".into(),
        instance_id: "instance".into(),
        moved: vec![MovedUnit {
            unit: "requirements".into(),
            stored: "old".into(),
            live: "new".into(),
        }],
    };
    assert_eq!(error.safe(), expected);

    let unreadable = read_error(ReadRefusal::UnitUnreadable {
        unit: "rules".into(),
        path: "/home/user/repo/.provenance/rules.jsonl".into(),
        error: "permission denied".into(),
    });
    assert_eq!(
        serde_json::to_value(&unreadable).unwrap(),
        serde_json::to_value(ReadFailure::UnitUnreadable {
            unit: "rules".into()
        })
        .unwrap(),
        "the published failure must not carry the local path or the native error"
    );
}

#[test]
#[verifies("rule_cache_completion_names_its_stage_at_the_deadline", examples)]
fn a_timed_out_cache_close_names_its_stage_and_drops_the_lock_path() {
    let lock = "/home/user/repo/.provenance/cache/provenance.db.close.lock";
    let error = ReadError(
        anyhow::Error::new(CompletionTimedOut {
            deadline: Duration::from_secs(10),
            stage: Stage::CloseLock(lock.into()),
        })
        .context("close the read cache"),
    );
    assert_eq!(
        error.safe(),
        ReadFailure::CacheCloseTimedOut {
            stage: CacheCloseStage::CloseLock
        }
    );
    assert_eq!(error.status(), 503);
    let published = serde_json::to_value(&error).unwrap();
    assert_eq!(
        published,
        serde_json::json!({"kind":"cache_close_timed_out","stage":"close_lock"})
    );
    assert!(!published.to_string().contains(lock), "{published}");

    let teardown = read_error(CompletionTimedOut {
        deadline: Duration::from_secs(10),
        stage: Stage::Teardown { connections: 2 },
    });
    assert_eq!(
        serde_json::to_value(&teardown).unwrap(),
        serde_json::json!({"kind":"cache_close_timed_out","stage":"teardown"})
    );
}

#[test]
fn a_common_refusal_raised_inside_a_read_keeps_its_kind_and_status() {
    let error = read_error(OperationFailure::InvalidInput {
        field: Some("offset".into()),
        reason: InvalidInputReason::InvalidValue,
    });
    assert_eq!(error.status(), 400);
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({"kind":"invalid_input","field":"offset","reason":"invalid_value"})
    );
}

/// The cause of a read failure of no known kind gives a path relative to the
/// repository root, on both separators.
#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn a_read_failure_of_no_known_kind_gives_its_cause_relative_to_the_repository() {
    for (root, path) in [
        (
            "/tmp/a /b/repo",
            "/tmp/a /b/repo/.provenance/state/scopes/default/requirements/req.jsonl",
        ),
        (
            r"\\?\C:\Users\Jo Smith\repo",
            r"\\?\C:\Users\Jo Smith\repo\.provenance\state\scopes\default/requirements/req.jsonl",
        ),
    ] {
        let error = ReadError(
            anyhow::anyhow!("read {path}: Is a directory (os error 21)")
                .context("failed to load requirements"),
        )
        .in_repository(camino::Utf8Path::new(root));
        assert_eq!(error.status(), 500);
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({
                "kind": "read_failed",
                "cause": "failed to load requirements: read .provenance/state/scopes/\
                          default/requirements/req.jsonl: Is a directory (os error 21)",
            }),
            "{path}"
        );
    }
}

/// A store refusal on a read keeps the kind of the same name: a read kind or
/// a common kind.
#[test]
#[verifies("rule_caller_refusal_has_declared_kind", examples)]
fn a_store_refusal_inside_a_read_keeps_its_kind_and_status() {
    use crate::write_error::{SourceFailure, WriteFailure};
    let refusal = |failure| {
        ReadError(
            SourceFailure::wrap(failure, anyhow::anyhow!("the store refused"))
                .context("the read loads its record"),
        )
    };
    let cases = [
        (
            refusal(WriteFailure::ResourceNotFound),
            serde_json::json!({"kind":"resource_not_found"}),
            404,
        ),
        (
            refusal(WriteFailure::invalid_field("request.id")),
            serde_json::json!({"kind":"invalid_input","field":"request.id","reason":"invalid_value"}),
            400,
        ),
        (
            refusal(WriteFailure::access_denied()),
            serde_json::json!({"kind":"access_denied"}),
            403,
        ),
    ];
    for (error, wire, status) in cases {
        assert_eq!(serde_json::to_value(&error).unwrap(), wire, "{error}");
        assert_eq!(error.status(), status, "{error}");
    }
}

/// A read or a write whose section entry cannot install a held state file
/// answers `state_file_in_use` with the path of the file relative to the
/// repository, and the status 503.
#[test]
#[verifies("rule_held_record_file_refusal_names_the_file", examples)]
fn a_state_file_in_use_names_the_file_and_has_the_status_503() {
    let held = || {
        anyhow::Error::new(crate::storage::local::StateFileInUse {
            path: Utf8PathBuf::from("/repo/.provenance/state/scopes/default/rules/rule.jsonl"),
            file: ".provenance/state/scopes/default/rules/rule.jsonl".to_owned(),
            source: std::io::Error::from_raw_os_error(32),
        })
        .context("enter the publication section")
    };
    let wire = serde_json::json!({
        "kind": "state_file_in_use",
        "file": ".provenance/state/scopes/default/rules/rule.jsonl"
    });
    let read = ReadError(held());
    assert_eq!(serde_json::to_value(&read).unwrap(), wire);
    assert_eq!(read.status(), 503);
    let write = crate::write_error::WriteError(held());
    assert_eq!(serde_json::to_value(&write).unwrap(), wire);
    assert_eq!(write.status(), 503);
}
