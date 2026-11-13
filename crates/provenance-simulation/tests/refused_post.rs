//! A message post whose publish is refused lands no Thread and no Message.
//!
//! The fault meets the post at each number of turns from its start, so it
//! meets each publish of the post.

use provenance_macros::verifies;
use provenance_simulation::generate::{Case, Fault, Operation, Step};
use provenance_simulation::{laws, report, run_case, Config, Run};
use support::replay::{replay_of_a_broken_probe, report_of_a_failed_check, selection, PROBE};

mod support {
    pub mod replay;
}

/// The most turns of the post before the refusal arms.
const MOST_TURNS: usize = 8;

const THREADS: &str = "scopes/default/threads/threads.jsonl";

const fn post() -> Step {
    Step::Write {
        engine: 0,
        op: Operation::PostMessage,
    }
}

/// A post from engine 0, `turns` turns of that engine, then a refusal of
/// the next publish.
fn refused_post(turns: usize) -> Vec<Step> {
    let mut steps = vec![post()];
    steps.extend(std::iter::repeat_n(Step::Turn(0), turns));
    steps.push(Step::Fault(Fault::Refuse));
    steps.extend(std::iter::repeat_n(Step::Turn(0), 200));
    steps
}

/// The rows of the Thread shard and the rows of the Message shards.
fn rows(run: &Run) -> (usize, usize) {
    let mut threads = 0;
    let mut messages = 0;
    for (key, bytes) in &run.entries {
        let key = key.as_str();
        if !key.starts_with("scopes/default/threads/") {
            continue;
        }
        let count = bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .count();
        if key == THREADS {
            threads += count;
        } else {
            messages += count;
        }
    }
    (threads, messages)
}

/// Fails `probe` when `run` breaks a law. The detail of the report starts
/// with the number of turns.
fn keep_the_laws(probe: &report::Probe<'_>, turns: usize, run: &Run) {
    let verdict = laws::check(run).map_err(|mut broken| {
        broken.detail = format!("turns {turns}: {}", broken.detail);
        broken
    });
    probe.laws(verdict);
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_refused_post_lands_no_thread_and_no_message() {
    let test = "a_refused_post_lands_no_thread_and_no_message";
    report::probe("engine", test, |probe| {
        let mut refused = 0;
        for turns in 0..=MOST_TURNS {
            let case = Case {
                engines: 2,
                steps: refused_post(turns),
            };
            let run = run_case(&case, Config::default());
            keep_the_laws(probe, turns, &run);
            if run.done[0].result.is_err() {
                refused += 1;
                assert_eq!(rows(&run), (0, 0), "turns {turns}");
            }
        }
        assert!(refused > 0, "the refusal met no post");
    });
}

// No Rule: a probe that the replay line of a probe in this test target runs
// this test target.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn the_replay_line_of_a_probe_runs_this_test_target() {
    let replay = replay_of_a_broken_probe("engine");
    let selection = selection(env!("CARGO_PKG_NAME"), env!("CARGO_CRATE_NAME"));
    assert!(replay.ends_with(&selection), "{replay}");
}

// No Rule: a probe that a failed check of a probe that is not a law reports
// the probe and its replay line in the form that the nightly report reads.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_failed_check_of_a_probe_reports_the_probe_and_its_replay_line() {
    let text = report_of_a_failed_check("engine");
    let (first, replay) = text.split_once('\n').expect("two lines");
    let start = format!("simulation probe {PROBE} failed a check in layer engine: ");
    assert!(first.starts_with(&start), "{text}");
    assert!(first.contains("a planted check"), "{text}");
    let selection = selection(env!("CARGO_PKG_NAME"), env!("CARGO_CRATE_NAME"));
    assert!(
        replay.starts_with("replay: ") && replay.ends_with(&selection),
        "{text}"
    );
}
