#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_store::{layout::ProvenanceLayout, publication::publication_guard};
use review_host_support::{
    events::{source, source_before_host, Socket},
    repository, request, response, start,
};
use serde_json::Value;
use std::io::Read;

fn layout(root: &std::path::Path) -> ProvenanceLayout {
    ProvenanceLayout::new(camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap())
}

fn closes(socket: &mut Socket) {
    let mut head = [0; 2];
    socket.0.read_exact(&mut head).unwrap();
    assert_eq!(
        head[0], 0x88,
        "unavailable observation closes the socket after reset"
    );
}

fn current_source(host: &review_host_support::Host, id: &str) -> Value {
    serde_json::from_str(
        &request(host, "GET", &format!("/sources/{id}"), true)
            .call()
            .unwrap()
            .into_string()
            .unwrap(),
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
#[test]
// Implementation aid: the notification-loss Rule remains a plan proposal without a version field.
fn real_kernel_queue_loss_sends_reset_and_current_read() {
    let repo = repository();
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    let ready = socket.frame();
    let count: usize = std::fs::read_to_string("/proc/sys/fs/inotify/max_queued_events")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        count <= 262_144,
        "a dedicated overflow runner is required for this queue size"
    );
    host.signal("-STOP");
    for index in 0..count + 1024 {
        let path = repo
            .path()
            .join(format!(".provenance/state/event-{index}.noise"));
        std::fs::write(&path, "event").unwrap();
        std::fs::remove_file(path).unwrap();
    }
    source(&host, repo.path(), "source_after_loss");
    host.signal("-CONT");
    let reset = loop {
        let frame = socket.frame();
        if frame["type"] == "reset" {
            break frame;
        }
        assert_eq!(frame["type"], "changed");
    };
    assert_eq!(reset["epoch"], ready["epoch"]);
    assert_eq!(reset["reason"], "file-events-lost");
    assert!(reset["sequence"].as_u64().unwrap() > ready["sequence"].as_u64().unwrap());
    assert_eq!(
        current_source(&host, "source_after_loss")["data"]["id"],
        "source_after_loss"
    );
}

#[tokio::test]
// Implementation aid: a failed canonical read must explicitly invalidate connected readers.
async fn failed_snapshot_resets_disconnects_and_recovers_on_a_file_event() {
    let repo = repository();
    source_before_host(repo.path(), "source_saved");
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    let ready = socket.frame();
    let layout = layout(repo.path());
    let shard = layout.scopes_dir().join("default/sources/source.jsonl");
    let original = std::fs::read(&shard).unwrap();
    let guard = publication_guard(&layout).await.unwrap();
    let temporary = shard.with_extension("replacement");
    std::fs::write(&temporary, "not JSON\n").unwrap();
    std::fs::rename(&temporary, &shard).unwrap();
    drop(guard);
    let reset = socket.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["reason"], "observation-failed");
    assert_eq!(reset["epoch"], ready["epoch"]);
    closes(&mut socket);
    let guard = publication_guard(&layout).await.unwrap();
    std::fs::write(&temporary, original).unwrap();
    std::fs::rename(temporary, shard).unwrap();
    drop(guard);
    let mut recovered = review_host_support::events::connect_when_available(&host);
    let ready = recovered.frame();
    assert_eq!(ready["epoch"], reset["epoch"]);
    assert!(ready["sequence"].as_u64().unwrap() > reset["sequence"].as_u64().unwrap());
    assert_eq!(
        current_source(&host, "source_saved")["data"]["id"],
        "source_saved"
    );
    source(&host, repo.path(), "source_after_repair");
    assert_eq!(
        recovered.frame()["scopes"][0]["records"][0]["id"],
        "source_after_repair"
    );
}

#[tokio::test]
// Implementation aid: removal and replacement exercise native watch attachment through the real host.
async fn removed_state_resets_then_replacement_reattaches_to_current_records() {
    let repo = repository();
    source_before_host(repo.path(), "source_old");
    let replacement = repository();
    source_before_host(replacement.path(), "source_new");
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    socket.frame();
    let layout = layout(repo.path());
    let old = layout.provenance_dir().join("old-state");
    let guard = publication_guard(&layout).await.unwrap();
    std::fs::rename(layout.state_dir(), &old).unwrap();
    drop(guard);
    let reset = socket.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["reason"], "observation-failed");
    closes(&mut socket);
    let guard = publication_guard(&layout).await.unwrap();
    std::fs::rename(
        replacement.path().join(".provenance/state"),
        layout.state_dir(),
    )
    .unwrap();
    drop(guard);
    let mut recovered = review_host_support::events::connect_when_available(&host);
    let ready = recovered.frame();
    assert_eq!(ready["epoch"], reset["epoch"]);
    assert_eq!(
        current_source(&host, "source_new")["data"]["id"],
        "source_new"
    );
    assert_eq!(
        response(request(&host, "GET", "/sources/source_old", true).call()).status(),
        404
    );
    source(&host, repo.path(), "source_after_replacement");
    assert_eq!(
        recovered.frame()["scopes"][0]["records"][0]["id"],
        "source_after_replacement"
    );
}

#[cfg(unix)]
#[test]
// Implementation aid: a nonregular canonical shard must not block the observer's read forever.
fn a_nonregular_shard_resets_without_waiting_for_fifo_data() {
    let repo = repository();
    source_before_host(repo.path(), "source_fifo");
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    socket.frame();
    let shard = repo
        .path()
        .join(".provenance/state/scopes/default/sources/source.jsonl");
    std::fs::remove_file(&shard).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(&shard)
        .status()
        .unwrap()
        .success());
    let reset = loop {
        let frame = socket.frame();
        if frame["type"] == "reset" {
            break frame;
        }
        assert_eq!(frame["type"], "changed");
    };
    assert_eq!(reset["reason"], "observation-failed");
    closes(&mut socket);
}
