//! A message post whose publish is refused lands no Thread and no Message.
//!
//! The fault meets the post at each number of turns from its start, so it
//! meets each publish of the post. CI does not run these tests: the ci
//! profile of nextest leaves out this crate.

use provenance_macros::verifies;
use provenance_simulation::generate::{Case, Fault, Operation, Step};
use provenance_simulation::{laws, run_case, Config, Run};

/// The most turns of the post before the refusal arms.
const MOST_TURNS: usize = 8;

const THREADS: &str = "scopes/default/threads/threads.jsonl";

fn post(engine: u8) -> Step {
    Step::Write {
        engine,
        op: Operation::PostMessage,
    }
}

/// A post from engine 0, `turns` turns of that engine, then a refusal of
/// the next publish.
fn refused_post(turns: usize) -> Vec<Step> {
    let mut steps = vec![post(0)];
    steps.extend(std::iter::repeat_n(Step::Turn(0), turns));
    steps.push(Step::Fault(Fault::Refuse));
    steps.extend(std::iter::repeat_n(Step::Turn(0), 200));
    steps
}

fn run_steps(steps: Vec<Step>) -> Run {
    run_case(&Case { engines: 2, steps }, Config::default())
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

fn keep_the_laws(turns: usize, run: &Run) {
    if let Err(broken) = laws::check(run) {
        panic!("turns {turns}: law {}: {}", broken.law, broken.detail);
    }
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_refused_post_lands_no_thread_and_no_message() {
    let mut refused = 0;
    for turns in 0..=MOST_TURNS {
        let run = run_steps(refused_post(turns));
        keep_the_laws(turns, &run);
        if run.done[0].result.is_err() {
            refused += 1;
            assert_eq!(rows(&run), (0, 0), "turns {turns}");
        }
    }
    assert!(refused > 0, "the refusal met no post");
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_post_after_a_refused_post_makes_one_thread_and_one_message() {
    let mut refused = 0;
    for turns in 0..=MOST_TURNS {
        let mut steps = refused_post(turns);
        steps.push(post(1));
        steps.extend(std::iter::repeat_n(Step::Turn(1), 200));
        let run = run_steps(steps);
        keep_the_laws(turns, &run);
        if run.done[0].result.is_err() {
            refused += 1;
            let second = &run.done[1];
            assert!(second.result.is_ok(), "turns {turns}: {second:?}");
            assert_eq!(rows(&run), (1, 1), "turns {turns}");
        }
    }
    assert!(refused > 0, "the refusal met no post");
}
