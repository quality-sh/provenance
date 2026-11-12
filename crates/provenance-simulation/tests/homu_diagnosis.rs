//! Temporary diagnosis of L3 and L4 failures of the engine layer. Not for
//! merge. It prints the shrunk case of each seed and the state of each case.

use hegel::{Hegel, Settings, TestCase, Verbosity};
use provenance_simulation::generate::{Case, Operation, Step};
use provenance_simulation::{check_case, laws, open_entries, report, run_case, scope, Config, Run};
use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

fn describe(name: &str, case: &Case, run: &Run) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== {name}");
    if case.steps.len() <= 20 {
        let _ = writeln!(out, "case: {}", serde_json::to_string(case).unwrap());
    }
    for done in &run.done {
        let _ = writeln!(
            out,
            "done: engine {:?} op#{} {:?} -> {:?} kind {:?} stopped {}",
            done.engine, done.operation, done.op, done.result, done.kind, done.stopped
        );
    }
    for (index, publish) in run.publishes.iter().enumerate() {
        let _ = writeln!(out, "publish {index}: {publish:?}");
    }
    let repository = open_entries(&run.entries).unwrap();
    let view = repository.view().unwrap();
    for topic in view.list_topics(&scope()).unwrap() {
        let _ = writeln!(
            out,
            "topic {} status {:?} claimed_by {:?}",
            topic.id.as_str(),
            topic.status,
            topic.claimed_by
        );
    }
    for requirement in view.list_requirements(&scope()).unwrap() {
        let _ = writeln!(
            out,
            "requirement {} status {:?} description {:?}",
            requirement.id.as_str(),
            requirement.status,
            requirement.description
        );
    }
    let _ = writeln!(out, "laws: {:?}", laws::check(run));
    out
}

fn seeded(seed: u64) -> String {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let sink: Arc<dyn Fn(&str) + Send + Sync> =
        Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_owned()));
    let outcome = hegel::with_output_override(sink, || {
        catch_unwind(AssertUnwindSafe(|| {
            Hegel::new(|tc: TestCase| {
                check_case("engine", &Case::draw(&tc), Config::default());
            })
            .settings(
                Settings::new()
                    .seed(Some(seed))
                    .database(None)
                    .test_cases(100)
                    .verbosity(Verbosity::Normal),
            )
            .run();
        }))
    });
    let text = match outcome {
        Err(payload) => report::panic_text(&*payload),
        Ok(()) => "every law held".to_owned(),
    };
    let lines = lines.lock().unwrap().join("\n");
    format!("=== seed {seed}\n{text}\n--- hegel output\n{lines}\n")
}

#[test]
fn diagnose() {
    let mut out = String::new();
    let case_a = Case {
        engines: 2,
        steps: vec![
            Step::Write {
                engine: 0,
                op: Operation::ArchiveRequirement { n: 0 },
            },
            Step::OutsideWrite(Operation::ClaimTopic),
        ],
    };
    out += &describe("case A", &case_a, &run_case(&case_a, Config::default()));
    let settle = |engine: u8| std::iter::repeat_n(Step::Turn(engine), 200);
    let update = |stale| Operation::UpdateRequirement { n: 0, stale };
    let mut archive_then_claim = vec![Step::Write { engine: 0, op: Operation::ArchiveRequirement { n: 0 } }];
    archive_then_claim.extend(settle(0));
    archive_then_claim.push(Step::OutsideWrite(Operation::ClaimTopic));
    let mut two_engine_updates = vec![Step::Write { engine: 0, op: update(false) }];
    two_engine_updates.extend(settle(0));
    two_engine_updates.push(Step::Write { engine: 1, op: update(true) });
    two_engine_updates.extend(settle(1));
    let mut engine_then_outside = vec![Step::Write { engine: 0, op: update(false) }];
    engine_then_outside.extend(settle(0));
    engine_then_outside.push(Step::OutsideWrite(update(true)));
    let moved_update = vec![Step::WriteOnMovedBase { engine: 0, op: update(false), outside: update(false) }];
    let moved_create = vec![Step::WriteOnMovedBase {
        engine: 0,
        op: Operation::CreateRequirement { n: 0 },
        outside: Operation::ClaimTopic,
    }];
    for (name, steps) in [
        ("archive, then outside claim", archive_then_claim),
        ("engine update, then engine stale update", two_engine_updates),
        ("engine update, then outside stale update", engine_then_outside),
        ("update on moved base, outside update", moved_update),
        ("create on moved base, outside claim", moved_create),
    ] {
        let case = Case { engines: 2, steps };
        out += &describe(name, &case, &run_case(&case, Config::default()));
    }
    for seed in [1, 7, 1234] {
        let text = seeded(seed);
        out += &text;
    }
    println!("{out}");
    panic!("diagnosis output:\n{out}");
}
