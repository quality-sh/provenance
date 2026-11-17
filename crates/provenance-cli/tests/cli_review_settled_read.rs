//! A graph file that an outside writer saves in place in two steps: the review host reports
//! a fault of the file only when a read after the file stops changing gives the fault.
use crate::review_host_support;
use provenance_macros::verifies;
use review_host_support::{
    events::Socket,
    follow::{page_rule, setup},
    manifest::{self, damaged_reason, save_in_two_steps, DAMAGED},
    repository, start,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[test]
#[verifies("rule_review_host_reads_settled_state", examples)]
fn a_write_caught_half_done_gives_no_fault() {
    let (repo, host, follow, _) = setup();
    let path = manifest::path(repo.path());
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

/// The reported fault is the fault of the complete file. On `main` this test is red because
/// the reason is the fault of the empty file that the first step leaves ("EOF while parsing a
/// value"). It is not red for the count: on `main` the follow stream also ends after its
/// first read-error.
#[test]
#[verifies("rule_review_host_reads_settled_state", examples)]
fn a_fault_of_the_settled_file_is_given_one_time() {
    let (repo, _host, mut follow, _) = setup();

    save_in_two_steps(&manifest::path(repo.path()), DAMAGED);

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
    let path = manifest::path(repo.path());
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

#[test]
#[verifies("rule_review_host_reads_settled_state", examples)]
fn a_write_caught_half_done_resets_the_review_events_socket_as_recovered() {
    let repo = repository();
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    let ready = socket.frame();
    let path = manifest::path(repo.path());
    let text = std::fs::read_to_string(&path).unwrap();

    save_in_two_steps(&path, &text);

    let reset = socket.frame();
    assert_eq!(reset["type"], "reset", "{reset}");
    assert_eq!(reset["reason"], "observation-recovered", "{reset}");
    assert_eq!(reset["epoch"], ready["epoch"]);
}
