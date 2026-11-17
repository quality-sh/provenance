//! A read on the HTTP route while a different process holds a record file
//! that a write must replace. On Linux and macOS a rename always replaces an
//! open file, so the test runs only on Windows.
#![cfg(all(windows, feature = "test-fixture"))]

use crate::support;

use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, host};

const PARENT: &str = "/requirements/req_shared/discussions";

/// Starts a Discussion on `req_shared`.
async fn discuss(host: &StatementHost, body: &str) {
    let (status, started) = call(
        host,
        "POST",
        PARENT,
        Some(json!({"data":{
            "actor":"reviewer", "declared_by":null, "role":"user", "body":body
        }})),
    )
    .await;
    assert_eq!(status, 200, "{started}");
}

/// The test keeps the Message shard open with a share mode that lets other
/// handles only read. The second Discussion succeeds, because its marker
/// names all of its changes. The next read enters the section and cannot
/// complete the install, so it answers 503 with `state_file_in_use` and the
/// file below the repository. When the holder closes the file, the next read
/// completes the write.
#[tokio::test]
#[verifies("rule_held_record_file_refusal_names_the_file", examples)]
async fn a_read_gives_state_file_in_use_while_a_holder_keeps_a_record_file() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    discuss(&host, "First text.").await;
    let threads = repo.layout.state_dir().join("scopes/default/threads");
    let shard = std::fs::read_dir(&threads)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.as_bytes().first().is_some_and(u8::is_ascii_digit))
        .expect("the Message shard of the month exists");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(threads.join(&shard))
        .unwrap();

    discuss(&host, "Second text.").await;
    let (status, refused) = call(&host, "GET", PARENT, None).await;
    drop(held);

    assert_eq!(status, 503, "{refused}");
    assert_eq!(
        refused["error"],
        json!({
            "kind": "state_file_in_use",
            "file": format!(".provenance/state/scopes/default/threads/{shard}")
        })
    );
    let (status, read) = call(&host, "GET", PARENT, None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(
        read.to_string().matches("Second text.").count(),
        1,
        "{read}"
    );
}
