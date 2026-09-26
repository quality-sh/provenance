use super::PublishError;
use camino::Utf8PathBuf;
use std::error::Error;
use std::io;

fn path(text: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(text)
}

fn io_error(text: &str) -> io::Error {
    io::Error::other(text.to_owned())
}

#[test]
fn each_refusal_names_the_path_and_what_the_operator_can_do() {
    let cases = [
        (
            PublishError::InvalidOutputPath {
                path: path("wiki"),
                detail: "empty".into(),
            },
            "invalid wiki output path wiki: empty",
        ),
        (
            PublishError::OutputSymlink { path: path("wiki") },
            "refusing wiki output symlink wiki; choose a real directory",
        ),
        (
            PublishError::OutputNotDirectory { path: path("wiki") },
            "refusing non-directory wiki output wiki; move it before publishing",
        ),
        (
            PublishError::CustomOutputUnrecognized { path: path("wiki") },
            "refusing nonempty custom wiki output wiki: it has no recognized \
             .provenance-wiki-output.json marker; move the directory, empty it, or use an \
             output previously created by Provenance",
        ),
        (
            PublishError::InvalidManifest {
                path: path("wiki/m.json"),
                detail: "not JSON".into(),
            },
            "unrecognized wiki ownership manifest wiki/m.json: not JSON",
        ),
        (
            PublishError::UnknownManifestVersion {
                path: path("wiki/m.json"),
                version: 9,
            },
            "unsupported wiki ownership manifest version 9 at wiki/m.json; upgrade Provenance \
             or choose another output",
        ),
        (
            PublishError::InvalidRoute {
                route: "../x".into(),
                detail: "parent step".into(),
            },
            "unsafe generated wiki route \"../x\": parent step",
        ),
        (
            PublishError::OutputChanged {
                path: path("wiki"),
                detail: "identity moved".into(),
            },
            "wiki output wiki changed during publication (identity moved); no replacement was \
             attempted",
        ),
        (
            PublishError::io("open", &path("wiki"), io_error("denied")),
            "failed to open wiki: denied",
        ),
        (
            PublishError::ReplacementRolledBack {
                output: path("wiki"),
                source: io_error("busy"),
            },
            "failed to install the completed wiki at wiki; the previous output was restored: \
             busy",
        ),
        (
            PublishError::RollbackFailed {
                output: path("wiki"),
                backup: path("wiki.bak"),
                install: io_error("busy"),
                rollback: io_error("gone"),
            },
            "wiki installation at wiki failed (busy) and rollback also failed (gone); output \
             ownership is ambiguous, so no cleanup was attempted; inspect backup wiki.bak",
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn an_artifact_refusal_lists_every_path_and_marks_an_unsafe_lock() {
    let without_lock = PublishError::AmbiguousArtifacts {
        paths: vec![path("a.lock"), path("a.stage")],
        unsafe_lock: None,
    };
    assert_eq!(
        without_lock.to_string(),
        "wiki publication cannot safely continue because transaction artifacts are present: \
         a.lock, a.stage; inspect them and explicitly move or remove them before retrying"
    );

    let with_lock = PublishError::AmbiguousArtifacts {
        paths: vec![path("a.lock")],
        unsafe_lock: Some(path("a.lock")),
    };
    assert!(
        with_lock.to_string().ends_with(
            "; the entry at a.lock is not a regular file, so something other than a \
             publication lock is standing where the lock goes"
        ),
        "{with_lock}"
    );
}

#[test]
fn a_cleanup_failure_keeps_the_primary_refusal_first_and_as_its_source() {
    let error = PublishError::CleanupFailed {
        primary: Box::new(PublishError::OutputSymlink { path: path("wiki") }),
        path: path("wiki.lock"),
        cleanup: io_error("denied"),
    };
    assert_eq!(
        error.to_string(),
        "refusing wiki output symlink wiki; choose a real directory; cleanup of \
         publisher-owned artifact wiki.lock also failed: denied"
    );
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("refusing wiki output symlink wiki; choose a real directory")
    );
    assert!(PublishError::OutputSymlink { path: path("wiki") }
        .source()
        .is_none());
}
