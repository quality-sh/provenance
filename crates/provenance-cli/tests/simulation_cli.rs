//! The cases of the simulation through the real CLI on disk.
//!
//! Each write is one `provenance api` process on a fixture repository. A
//! `Turn` starts the next two writes at once. A `Stop` stops a write process.
//! An outside write runs on a git branch, and git merges it back through
//! `provenance merge-jsonl`. The run keeps these laws:
//!
//! | Law | Statement |
//! | --- | --------- |
//! | L3 | A write that succeeded stays unless a later write changed it. |
//! | L4 | Of two writes on one etag, at most one succeeds. |
//! | L6 | The reads at the end equal the reads after a full rebuild. |
//! | L7 | Each answer of one writer has the answer kind of the engine. |
//!
//! The nightly workflow runs this target on Linux, macOS and Windows. Two
//! processes at once make a case nondeterministic, so Hegel only warns.

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/catch_up_steps.rs"]
    pub mod catch_up_steps;
    #[path = "../../../provenance-transport/tests/support/transcript.rs"]
    pub mod transcript;
}

#[path = "simulation_cli/laws.rs"]
mod laws;
#[path = "simulation_cli/process.rs"]
mod process;

use hegel::{Hegel, NondeterminismStrictness, Settings, TestCase};
use laws::{check, CliRun, Ran};
use process::{binary, commit, etag, git, now, run_at_once, Reads};
use provenance_simulation::generate::{
    Case, Context, Fault, Operation, Step, Task, FIRST_REQUIREMENT,
};
use provenance_simulation::{report, run_case, Config, Done};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::storage::StateKey;
use provenance_transport::fixture::records::Repository;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use support::transcript::{assert_same, copy_without_projection, read_all};

/// The arguments of the merge driver after the program.
const MERGE_ARGUMENTS: &str = "merge-jsonl %O %A %B --output %A --path %P";

/// One fixture repository with the initial state of the simulation, and
/// the case that runs on it.
struct Fixture {
    repo: Repository,
    branches: tempfile::TempDir,
    base: String,
    context: Context,
    serial: u64,
    engines: u8,
    groups: Vec<(Ran, Vec<Done>)>,
}

impl Fixture {
    fn new(engines: u8) -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let root = dir.path();
        let empty = Case {
            engines: 1,
            steps: Vec::new(),
        };
        for (key, bytes) in run_case(&empty, Config::default()).entries {
            let path = root.join(".provenance/state").join(key.as_str());
            let parent = path.parent().expect("a parent");
            std::fs::create_dir_all(parent).expect("a directory");
            std::fs::write(path, bytes).expect("a state file");
        }
        let attributes = ".provenance/state/**/*.jsonl merge=provenance-jsonl\n";
        std::fs::write(root.join(".gitattributes"), attributes).expect("a file");
        std::fs::write(root.join(".gitignore"), ".provenance/cache/\n").expect("a file");
        let program = binary().display().to_string().replace('\\', "/");
        let driver = format!("\"{program}\" {MERGE_ARGUMENTS}");
        for args in [
            &["init", "-q"][..],
            &["config", "core.autocrlf", "false"],
            &["config", "merge.provenance-jsonl.driver", &driver],
        ] {
            git(root, args).expect("the fixture opens");
        }
        commit(root, "initial state").expect("the fixture commits");
        let base = git(root, &["rev-parse", "HEAD"]).expect("a commit");
        let first_etag = etag(root, FIRST_REQUIREMENT).expect("an etag");
        let layout = ProvenanceLayout::new(root.to_str().expect("UTF-8"));
        Self {
            repo: Repository { dir, layout },
            branches: tempfile::tempdir().expect("a directory"),
            base,
            context: Context::new(first_etag),
            serial: 0,
            engines: engines.max(1),
            groups: Vec::new(),
        }
    }

    fn root(&self) -> PathBuf {
        self.repo.dir.path().to_owned()
    }

    /// The task of the next operation.
    fn task(&mut self, engine: Option<u8>, op: Operation) -> Task {
        self.serial += 1;
        let effects = self.groups.iter().flat_map(|(_, done)| done);
        let effects = effects.filter_map(|done| done.result.clone().ok());
        self.context.learn(effects);
        let slot = engine.map(|engine| usize::from(engine % self.engines));
        self.context.task(self.serial, slot, op)
    }

    /// Runs the writes of `pending` in the way `way`.
    fn run(&mut self, way: Ran, pending: Vec<(u8, Operation)>, stop: Option<u64>) {
        let mut tasks = Vec::new();
        for (engine, op) in pending {
            tasks.push(self.task(Some(engine), op));
        }
        let done = run_at_once(&self.root(), &tasks, stop);
        if !done.is_empty() {
            self.groups.push((way, done));
        }
    }

    /// Runs `op` on a branch from the last merge and merges it back.
    fn write_outside(&mut self, op: Operation) {
        let task = self.task(None, op);
        let root = self.root();
        let branch = self.branches.path().join(task.operation.to_string());
        let branch_text = branch.to_str().expect("UTF-8");
        let add: [&str; 5] = ["worktree", "add", "--detach", branch_text, &self.base];
        let opened = commit(&root, "ours").and_then(|_| git(&root, &add));
        opened.expect("the branch opens");
        let mut outcome = run_at_once(&branch, std::slice::from_ref(&task), None);
        if let Some(Done { result: Ok(_), .. }) = outcome.first() {
            let merged = commit(&branch, "outside")
                .and_then(|_| git(&branch, &["rev-parse", "HEAD"]))
                .and_then(|theirs| git(&root, &["merge", "-q", "--no-edit", &theirs]));
            if let Err(conflict) = merged {
                let _ = git(&root, &["merge", "--abort"]);
                outcome[0].result = Err(format!("merge refused: {conflict}"));
            }
        }
        self.base = git(&root, &["rev-parse", "HEAD"]).expect("a commit");
        let _ = git(&root, &["worktree", "remove", "--force", branch_text]);
        if !outcome.is_empty() {
            self.groups.push((Ran::Outside, outcome));
        }
    }

    /// L6: the reads at the end equal the reads after a full rebuild.
    fn reads(&self) -> Result<(), String> {
        let reads = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let caught_up = Reads(self.root().display().to_string());
            let caught_up = now(read_all(&caught_up, &self.repo));
            let copy = copy_without_projection(&self.repo);
            let rebuilt = Reads(copy.dir.path().display().to_string());
            let rebuilt = now(read_all(&rebuilt, &copy));
            assert_same(&caught_up, &rebuilt, "the reads at the end of the case");
        }));
        reads.map_err(|payload| report::panic_text(&*payload))
    }
}

/// The step after the last one: it runs the writes that wait, one by one.
const END: Step = Step::Fault(Fault::Refuse);

/// Runs `case` on a new fixture repository.
fn run_cli(case: &Case) -> CliRun {
    let mut fixture = Fixture::new(case.engines);
    let engines = fixture.engines;
    let mut pending: Vec<(u8, Operation)> = Vec::new();
    for step in case.steps.iter().chain(std::iter::once(&END)) {
        if let Step::Write { engine, op } = step {
            pending.push((*engine, op.clone()));
            continue;
        }
        if matches!(step, Step::Turn(_)) && pending.len() >= 2 {
            let both = pending.drain(..2).collect();
            fixture.run(Ran::AtOnce, both, None);
        }
        let mut stopped = None;
        if let Step::Fault(Fault::Stop { engine }) = step {
            let slot = engine % engines;
            if let Some(index) = pending.iter().position(|(e, _)| e % engines == slot) {
                stopped = Some((pending.remove(index), u64::from(*engine) * 3 % 60));
            }
        }
        for one in std::mem::take(&mut pending) {
            fixture.run(Ran::Alone, vec![one], None);
        }
        if let Some((one, millis)) = stopped {
            fixture.run(Ran::Stopped, vec![one], Some(millis));
        }
        if let Step::OutsideWrite(op) = step {
            fixture.write_outside(op.clone());
        }
    }
    CliRun {
        reads: fixture.reads(),
        entries: state_entries(&fixture.root()),
        groups: fixture.groups,
    }
}

/// The canonical state files of the repository at `root`.
fn state_entries(root: &Path) -> BTreeMap<StateKey, Vec<u8>> {
    let state = root.join(".provenance/state");
    let mut found = BTreeMap::new();
    let mut directories = vec![state.clone()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).expect("a state directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                directories.push(path);
                continue;
            }
            let key = path.strip_prefix(&state).expect("a path in the state");
            let key = key.to_str().expect("UTF-8").replace('\\', "/");
            let key = StateKey::new(key).expect("a state key");
            found.insert(key, std::fs::read(&path).expect("a state file"));
        }
    }
    found
}

/// Runs `case` through the CLI and fails with the report of a broken law.
fn check_case(case: &Case) {
    let Err(violation) = check(case, &run_cli(case)) else {
        return;
    };
    let layer = format!("cli-{}", std::env::consts::OS);
    let seed = report::seed();
    let shown = seed.clone().unwrap_or_else(|| "<seed>".into());
    let replay = format!(
        "HEGEL_SEED={shown} cargo nextest run --profile simulation --run-ignored all -p provenance-cli --test simulation_cli"
    );
    let failure = report::Failure {
        layer: &layer,
        law: violation.law,
        detail: &violation.detail,
        seed,
        case,
        replay,
    };
    let json = serde_json::to_string_pretty(&failure).expect("a failure is JSON");
    if let Some(directory) = std::env::var_os("SIMULATION_FAILURE_DIR") {
        let path = Path::new(&directory).join(format!("{layer}-{}.json", violation.law));
        let written = std::fs::create_dir_all(&directory);
        if let Err(error) = written.and_then(|()| std::fs::write(path, &json)) {
            eprintln!("could not write the failure: {error}");
        }
    }
    let (law, detail, replay) = (violation.law, &violation.detail, &failure.replay);
    let text = format!("{detail}\nreplay: {replay}\nsteps: {json}");
    panic!("simulation law {law} broken in layer {layer}: {text}");
}

#[test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
fn random_cases_keep_the_laws_through_the_cli() {
    let settings = Settings::new().nondeterminism_strictness(NondeterminismStrictness::Warn);
    Hegel::new(|tc: TestCase| check_case(&Case::draw(&tc))).settings(settings).run();
}

const fn write(engine: u8, op: Operation) -> Step {
    Step::Write { engine, op }
}

const fn update(n: u8) -> Operation {
    Operation::UpdateRequirement { n, stale: false }
}

/// Probe: every operation with a public route writes alone through the CLI.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn each_operation_writes_through_the_cli() {
    let steps = vec![
        write(0, Operation::CreateRequirement { n: 0 }),
        write(1, update(1)),
        write(1, Operation::SubmitReview),
        write(1, Operation::WithdrawReview { n: 0 }),
        write(0, update(0)),
        write(0, Operation::ClaimTopic),
        write(0, Operation::ReleaseTopic),
    ];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let done: Vec<_> = run.groups.iter().flat_map(|(_, done)| done).collect();
    assert_eq!(done.len(), 7, "{done:?}");
    assert!(done.iter().all(|done| done.result.is_ok()), "{done:?}");
    check(&case, &run).unwrap();
}

/// Probe: two updates at once on one etag; one wins and the other conflicts.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn two_updates_at_once_have_one_winner() {
    let steps = vec![write(0, update(0)), write(1, update(0)), Step::Turn(0)];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let (way, done) = &run.groups[0];
    assert_eq!(*way, Ran::AtOnce);
    let won = done.iter().filter(|done| done.result.is_ok()).count();
    assert_eq!(won, 1, "{done:?}");
    check(&case, &run).unwrap();
}

/// Probe: a write on a branch merges through `provenance merge-jsonl`, and a
/// stopped write leaves a state that reads.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stopped_write_keep_the_laws() {
    let steps = vec![
        write(0, update(0)),
        Step::OutsideWrite(Operation::CreateRequirement { n: 0 }),
        write(1, Operation::ClaimTopic),
        Step::Fault(Fault::Stop { engine: 1 }),
    ];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let ways: Vec<_> = run.groups.iter().map(|(way, _)| *way).collect();
    assert_eq!(ways, [Ran::Alone, Ran::Outside, Ran::Stopped]);
    let (_, outside) = &run.groups[1];
    assert!(outside[0].result.is_ok(), "{outside:?}");
    check(&case, &run).unwrap();
}
