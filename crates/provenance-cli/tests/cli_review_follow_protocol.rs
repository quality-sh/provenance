#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::scripted_feed;

use review_host_support::{follow::create, repository};
use serde_json::json;

#[test]
// Implementation aid: a sequence gap from an independent peer must re-read saved state.
fn sequence_gap_returns_reset_even_when_the_document_has_not_changed() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    assert_eq!(follow.frame()["type"], "state");
    feed.send(json!({"type":"changed", "epoch":feed.epoch, "sequence":2, "scopes":[]}));
    let reset = follow.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["sequence"], 2);
    assert_eq!(reset["continuity"], "unknown");
    assert!(reset["records"].to_string().contains("req_follow"));
}

#[test]
// Implementation aid: an independent restarted peer must reset a saved client baseline.
fn changed_host_epoch_returns_reset_with_current_state() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    follow.frame();
    feed.send(json!({"type":"ready", "epoch":"restarted-host", "sequence":0}));
    let reset = follow.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["epoch"], "restarted-host");
    assert_eq!(reset["continuity"], "unknown");
    assert!(reset["records"].to_string().contains("req_follow"));
}

#[test]
// Implementation aid: reject unknown host tags instead of treating them as invalidations.
fn unknown_host_frame_is_a_protocol_error() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = scripted_feed::Feed::start(repo.path());
    feed.send(json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    follow.frame();
    feed.send(json!({"type":"unrecognized", "epoch":feed.epoch, "sequence":1}));
    let error = follow.frame();
    assert_eq!(error["type"], "disconnected");
    assert_eq!(error["continuity"], "unknown");
}
