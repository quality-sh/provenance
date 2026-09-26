use super::ReadError;
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::{MovedUnit as NativeMovedUnit, ReadRefusal};
use crate::stale::git::GitRefusal;
use camino::Utf8PathBuf;
use provenance_core::protocol::read_failure::{MovedUnit, ReadFailure};

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
            ReadFailure::ReadFailed,
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
            ReadFailure::ReadFailed,
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
