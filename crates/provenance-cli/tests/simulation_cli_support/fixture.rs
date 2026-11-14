//! The fixture repository of the CLI simulation and the run of one case on
//! it.

use crate::laws::{CliRun, Ran};
use crate::process::stop::{self, Stop};
use crate::process::{binary, commit, etag, git, layer, run_at_once, take_l0_breaks};
use crate::reads;
use provenance_simulation::generate::{
    requirement_id, Case, Context, Effect, Fault, Operation, Step, Task, FIRST_REQUIREMENT,
};
use provenance_simulation::{run_case, stats, Config, Done};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::storage::{StateKey, StateStorage};
use provenance_transport::fixture::records::Repository;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The arguments of the merge driver after the program.
const MERGE_ARGUMENTS: &str = "merge-jsonl %O %A %B --output %A --path %P";

/// The count of the updates sent with the etag of a read before a merge.
const MOVED_ETAG_UPDATES: &str = "updates with the etag of a read before a merge";
/// The count of those updates that refused.
const MOVED_ETAG_REFUSALS: &str = "updates with the etag of a read before a merge that refused";

/// When L6 is checked: after each write, or only at the end of a case when
/// `SIMULATION_L6` is `end`.
fn l6_after_each_write() -> bool {
    std::env::var("SIMULATION_L6").as_deref() != Ok("end")
}

/// One fixture repository with the initial state of the simulation, and
/// the case that runs on it.
pub struct Fixture {
    /// The repository on disk.
    pub repo: Repository,
    branches: tempfile::TempDir,
    /// The commit of the last merge.
    pub base: String,
    context: Context,
    serial: u64,
    engines: u8,
    /// Each group of writes and what its writes did.
    pub groups: Vec<(Ran, Vec<Done>)>,
    /// What each stop of the case did.
    pub stops: Vec<Stop>,
    /// The first L6 failure of the case.
    pub reads: Result<(), String>,
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
            stops: Vec::new(),
            reads: Ok(()),
        }
    }

    pub fn root(&self) -> PathBuf {
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
    fn run(&mut self, way: Ran, pending: Vec<(u8, Operation)>) {
        let mut tasks = Vec::new();
        for (engine, op) in pending {
            tasks.push(self.task(Some(engine), op));
        }
        self.run_tasks(way, &tasks);
    }

    /// Runs the write `op` of `engine` alone with a stop at `at`.
    fn run_stopped(&mut self, (engine, op): (u8, Operation), at: u8) {
        let task = self.task(Some(engine), op);
        let (done, stop) = stop::run_stopped(&self.root(), &self.repo.layout, &task, at);
        self.note(stop);
        if let Some(done) = done {
            self.groups.push((Ran::Stopped, vec![done]));
            self.check_reads();
        }
    }

    /// Counts what a stop did and keeps it.
    fn note(&mut self, stop: Stop) {
        stop.count();
        self.stops.push(stop);
    }

    /// Reads the etag of Requirement `n`, merges `outside` from a branch, and
    /// then sends an update of the Requirement with the etag of that read.
    /// So the update names an etag that the outside write can have moved.
    fn update_on_moved_etag(&mut self, engine: u8, n: u8, outside: Operation) {
        let read = etag(&self.root(), &requirement_id(n));
        self.write_outside(outside);
        let op = Operation::UpdateRequirement { n, stale: true };
        let mut task = self.task(Some(engine), op);
        if let Ok(etag) = read {
            task.stale_etag = etag;
        }
        let refused = self.run_tasks(Ran::Alone, &[task]);
        stats::add(&layer(), MOVED_ETAG_UPDATES, 1);
        stats::add(&layer(), MOVED_ETAG_REFUSALS, u64::from(refused));
    }

    /// Runs `tasks` in the way `way`; tells if a write refused.
    fn run_tasks(&mut self, way: Ran, tasks: &[Task]) -> bool {
        let done = run_at_once(&self.root(), tasks);
        let refused = done.iter().any(|done| done.result.is_err());
        if !done.is_empty() {
            self.groups.push((way, done));
            self.check_reads();
        }
        refused
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
        let mut outcome = run_at_once(&branch, std::slice::from_ref(&task));
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
            self.check_reads();
        }
    }

    /// The Requirements that a create of the case made.
    fn created(&self) -> Vec<String> {
        let mut created: Vec<String> = self
            .groups
            .iter()
            .flat_map(|(_, done)| done)
            .filter_map(|done| match &done.result {
                Ok(Effect::Created { id }) => Some(id.clone()),
                _ => None,
            })
            .collect();
        created.sort();
        created.dedup();
        created
    }

    /// L6 after the last group, when the case checks after each write.
    fn check_reads(&mut self) {
        if l6_after_each_write() {
            self.l6(&format!(
                "the reads after write group {}",
                self.groups.len()
            ));
        }
    }

    /// Checks L6 and keeps the first failure.
    pub fn l6(&mut self, context: &str) {
        if self.reads.is_err() {
            return;
        }
        let started = Instant::now();
        self.reads = reads::check_reads(&self.repo, &self.created(), context);
        let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        stats::add(&layer(), "L6 checks", 1);
        stats::add(&layer(), "L6 milliseconds", millis);
    }
}

/// Runs the steps of `case` on a new fixture repository.
pub fn run_steps(case: &Case) -> Fixture {
    take_l0_breaks();
    let mut fixture = Fixture::new(case.engines);
    let engines = fixture.engines;
    let mut pending: Vec<(u8, Operation)> = Vec::new();
    // `None` is the end of the case: it runs the writes that wait, one by one.
    // An update on a moved base keeps its step; another write on a moved base
    // is an outside write and then the write.
    let steps: Vec<Step> = case
        .steps
        .iter()
        .flat_map(|step| match step {
            Step::WriteOnMovedBase {
                op: Operation::UpdateRequirement { .. },
                ..
            } => vec![step.clone()],
            other => other.expand(),
        })
        .collect();
    for step in steps.iter().map(Some).chain(std::iter::once(None)) {
        if let Some(Step::Write { engine, op }) = step {
            pending.push((*engine, op.clone()));
            continue;
        }
        if matches!(step, Some(Step::Turn(_))) && pending.len() >= 2 {
            let both = pending.drain(..2).collect();
            fixture.run(Ran::AtOnce, both);
        }
        let mut stopped = None;
        if let Some(Step::Fault(Fault::Stop { engine, at })) = step {
            let slot = engine % engines;
            let index = pending.iter().position(|(e, _)| e % engines == slot);
            stopped = index.map(|index| (pending.remove(index), *at));
            if stopped.is_none() {
                fixture.note(Stop::NoWrite);
            }
        }
        for one in std::mem::take(&mut pending) {
            fixture.run(Ran::Alone, vec![one]);
        }
        // The stopped write runs alone, after the other waiting writes.
        if let Some((one, at)) = stopped {
            fixture.run_stopped(one, at);
        }
        if let Some(Step::OutsideWrite(op)) = step {
            fixture.write_outside(op.clone());
        }
        if let Some(Step::WriteOnMovedBase {
            engine,
            op: Operation::UpdateRequirement { n, .. },
            outside,
        }) = step
        {
            fixture.update_on_moved_etag(*engine, *n, outside.clone());
        }
    }
    fixture
}

impl Fixture {
    /// Ends the case: checks L6 at the end when the case does not check it
    /// after each write, counts the case, and gives what it did.
    pub fn finish(mut self, started: Instant) -> CliRun {
        if !l6_after_each_write() {
            self.l6("the reads at the end of the case");
        }
        let writes = self
            .groups
            .iter()
            .map(|(_, done)| done.len())
            .sum::<usize>();
        stats::count_run(&layer());
        stop::count_none();
        stats::add(&layer(), MOVED_ETAG_UPDATES, 0);
        stats::add(&layer(), MOVED_ETAG_REFUSALS, 0);
        stats::add(
            &layer(),
            "writes",
            u64::try_from(writes).unwrap_or(u64::MAX),
        );
        let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        stats::add(&layer(), "case milliseconds", millis);
        CliRun {
            l0_breaks: take_l0_breaks(),
            reads: self.reads.clone(),
            entries: state_entries(&self.root()),
            groups: self.groups,
            stops: self.stops,
        }
    }
}

/// Runs `case` on a new fixture repository.
pub fn run_cli(case: &Case) -> CliRun {
    let started = Instant::now();
    run_steps(case).finish(started)
}

/// The state of the repository at `root`, read through the local storage
/// adapter: the manifest and the shards, and no temporary file that a
/// killed write left. A read that fails gives its error.
pub fn state_entries(root: &Path) -> Result<BTreeMap<StateKey, Vec<u8>>, String> {
    let layout = ProvenanceLayout::new(root.to_str().expect("UTF-8"));
    provenance_store::storage::local::LocalStateStorage::new(layout)
        .read_view()
        .map(provenance_store::storage::StateView::into_entries)
        .map_err(|error| format!("{error:#}"))
}
