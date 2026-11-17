//! A graph file that an outside writer saves in place in two steps: the review host reports
//! a fault of the file only when a read after the file stops changing gives the fault.
use crate::review_host_support;
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use review_host_support::follow::{page_rule, setup};
use std::{
    io::Write as _,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const DAMAGED: &str = "{\"scopes\":\"default\"}\n";

fn manifest(root: &Path) -> PathBuf {
    root.join(".provenance/state/manifest.json")
}

fn damaged_reason() -> String {
    format!(
        "manifest.json line 1: invalid type: string \"default\", expected a sequence; \
         {STATE_CORRECTIONS}"
    )
}

/// Saves `text` in place as an editor does: the file is empty for a short time before the
/// text is in it.
fn save_in_two_steps(path: &Path, text: &str) {
    let mut file = std::fs::File::create(path).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    file.write_all(text.as_bytes()).unwrap();
}

#[test]
#[verifies("rule_review_host_reads_settled_state", examples)]
fn a_write_caught_half_done_gives_no_fault() {
    let (repo, host, follow, _) = setup();
    let path = manifest(repo.path());
    let text = std::fs::read_to_string(&path).unwrap();

    save_in_two_steps(&path, &text);
    page_rule(&host, "rule_after_save", "req_follow");

    loop {
        let frame = follow.frame();
        assert_ne!(frame["type"], "read-error", "{frame}");
        if frame.to_string().contains("rule_after_save") {
            break;
        }
    }
}

#[test]
#[verifies("rule_review_host_reads_settled_state", examples)]
fn a_fault_of_the_settled_file_is_given_one_time() {
    let (repo, _host, mut follow, _) = setup();

    save_in_two_steps(&manifest(repo.path()), DAMAGED);

    let frame = follow.frame();
    assert_eq!(frame["type"], "read-error", "{frame}");
    assert_eq!(frame["reason"], damaged_reason());
    assert!(!follow.child.wait().unwrap().success());
    follow.ended();
}

#[test]
#[verifies("rule_review_host_reads_settled_state", examples)]
fn a_file_that_continues_to_change_gets_an_answer() {
    let (repo, _host, follow, _) = setup();
    let path = manifest(repo.path());
    save_in_two_steps(&path, DAMAGED);
    let writing = Arc::new(AtomicBool::new(true));
    let writer = {
        let writing = writing.clone();
        std::thread::spawn(move || {
            while writing.load(Ordering::Acquire) {
                review_host_support::replace_file(&path, DAMAGED);
                std::thread::sleep(Duration::from_millis(50));
            }
        })
    };

    let frame = follow.frame();
    writing.store(false, Ordering::Release);
    writer.join().unwrap();

    assert_eq!(frame["type"], "read-error", "{frame}");
    assert_eq!(frame["reason"], damaged_reason());
}
