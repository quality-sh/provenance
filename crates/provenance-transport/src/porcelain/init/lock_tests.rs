//! Init and readers of the state: a read never sees the manifest moved away.
use super::{prepare_init, InitOptions, InitPlan};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Duration;

/// A step that takes longer than this is a fault. The test fails and does not hang.
const LIMIT: Duration = Duration::from_secs(60);

fn options(actor: &str) -> InitOptions {
    InitOptions {
        scope: Some("default".to_owned()),
        path_prefix: Some(Utf8PathBuf::from(".")),
        disposition_actor_ids: vec![actor.to_owned()],
        clear_disposition_actors: false,
        ste_pdf: None,
        invocation_channel: provenance_porcelain::init::InvocationChannel::Native,
        package_manager: None,
        skills: crate::skills::TEST_SKILLS,
    }
}

fn initialized() -> (tempfile::TempDir, Utf8PathBuf, ProvenanceLayout) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    prepare_init(&repo, options("reviewer"))
        .unwrap()
        .apply()
        .unwrap();
    let layout = ProvenanceLayout::new(repo.clone());
    (directory, repo, layout)
}

/// Whether a second holder can take the publication lock now.
fn lock_is_free(layout: &ProvenanceLayout) -> bool {
    use fs2::FileExt;
    let path = layout.publication_lock_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    let free = file.try_lock_exclusive().is_ok();
    if free {
        let _ = fs2::FileExt::unlock(&file);
    }
    free
}

fn manifest_text(layout: &ProvenanceLayout) -> String {
    String::from_utf8(std::fs::read(layout.manifest_path()).unwrap()).unwrap()
}

/// Opens the repository as the review follow and the commands do, on its own thread.
fn open_on_a_thread(repo: &Utf8Path) -> mpsc::Receiver<anyhow::Result<Vec<String>>> {
    let (sender, receiver) = mpsc::channel();
    let repo = repo.to_owned();
    std::thread::spawn(move || {
        let read = Repository::open(&repo, "default")
            .map_err(anyhow::Error::from)
            .and_then(|repository| repository.manifest())
            .map(|manifest| manifest.disposition_actor_ids);
        let _ = sender.send(read);
    });
    receiver
}

/// An init on its own thread that stopped when it first moved a file away.
struct StoppedInit {
    resume: mpsc::Sender<()>,
    finished: mpsc::Receiver<anyhow::Result<()>>,
}

impl StoppedInit {
    /// The first file that a second init replaces is the manifest.
    fn start(plan: InitPlan) -> Self {
        let (stopped_sender, stopped) = mpsc::channel();
        let (resume, resume_receiver) = mpsc::channel::<()>();
        let (finished_sender, finished) = mpsc::channel();
        std::thread::spawn(move || {
            let mut gate = Some((stopped_sender, resume_receiver));
            crate::test_probes::arm("file_displaced", move || {
                if let Some((stopped, resume)) = gate.take() {
                    let _ = stopped.send(());
                    let _ = resume.recv();
                }
            });
            let _ = finished_sender.send(plan.apply().map(|_| ()));
        });
        stopped
            .recv_timeout(LIMIT)
            .expect("init did not move the manifest away");
        Self { resume, finished }
    }

    fn resume(self) -> anyhow::Result<()> {
        let _ = self.resume.send(());
        self.finished
            .recv_timeout(LIMIT)
            .expect("init did not finish")
    }
}

/// While a second init has the manifest moved away, it holds the publication
/// lock, so a holder of the lock waits for init. The manifest lands when init
/// continues, and the lock is then free.
/// This test has no Rule binding: it does no read, and it checks the lock that
/// the read test depends on.
#[test]
fn second_init_holds_the_publication_lock_while_the_manifest_is_away() {
    let (_directory, repo, layout) = initialized();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();

    let init = StoppedInit::start(plan);
    let manifest_away = !layout.manifest_path().exists();
    let lock_free = lock_is_free(&layout);
    init.resume().unwrap();

    assert!(manifest_away, "init stopped with the manifest in place");
    assert!(
        !lock_free,
        "init moved the manifest away without the publication lock"
    );
    assert!(lock_is_free(&layout), "init kept the publication lock");
    let after = manifest_text(&layout);
    assert!(after.contains("new-reviewer"), "{after}");
}

/// A repository open, the read of the review follow and of the commands,
/// starts while a second init has the manifest moved away. The read waits for
/// init and gets the graph from after init; it does not get an error.
#[test]
#[provenance_macros::verifies("rule_read_waits_for_repeated_init", examples)]
fn a_read_while_init_moves_the_manifest_gets_a_graph() {
    let (_directory, repo, layout) = initialized();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();

    let init = StoppedInit::start(plan);
    // With the lock held, the read cannot end before init continues. With no
    // lock, the read ends while init is stopped.
    let read = if lock_is_free(&layout) {
        let read = open_on_a_thread(&repo).recv_timeout(LIMIT);
        init.resume().unwrap();
        read.expect("the read did not end")
    } else {
        let read = open_on_a_thread(&repo);
        init.resume().unwrap();
        read.recv_timeout(LIMIT).expect("the read did not end")
    };

    let actors = read.unwrap_or_else(|error| panic!("the read got an error: {error:#}"));
    assert_eq!(actors, ["new-reviewer"]);
}

/// Every file and folder under `root`, with the bytes of each file.
fn tree(root: &Utf8Path) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut entries = BTreeMap::new();
    let mut folders = vec![root.to_owned()];
    while let Some(folder) = folders.pop() {
        for entry in folder.read_dir_utf8().unwrap() {
            let path = entry.unwrap().into_path();
            let name = path.strip_prefix(root).unwrap().to_string();
            if path.is_dir() {
                entries.insert(name, None);
                folders.push(path);
            } else {
                entries.insert(name, Some(std::fs::read(&path).unwrap()));
            }
        }
    }
    entries
}

/// A second init fails after it moved the manifest away, while it holds the
/// publication lock. The files are as they were before that init, no new path
/// stays, and the lock is free: a following read and a following init work.
#[test]
#[provenance_macros::verifies("rule_init_apply_rolls_back_owned_changes", examples)]
fn a_failed_second_init_restores_the_state_and_frees_the_lock() {
    let (_directory, repo, layout) = initialized();
    // Init writes AGENTS.md again after the manifest. Every temporary name of
    // that write is taken, so init fails after the manifest moved.
    std::fs::remove_file(repo.join("AGENTS.md")).unwrap();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();
    let occupied: Vec<Utf8PathBuf> = (0..100_u8)
        .map(|attempt| {
            repo.join(format!(
                ".AGENTS.md.provenance-{}-{attempt}.tmp",
                std::process::id()
            ))
        })
        .collect();
    for path in &occupied {
        std::fs::write(path, "occupied\n").unwrap();
    }
    let before = tree(&repo);

    let held = std::rc::Rc::new(std::cell::Cell::new(None));
    let seen = std::rc::Rc::clone(&held);
    let probe_layout = layout.clone();
    crate::test_probes::arm("file_displaced", move || {
        if seen.get().is_none() {
            seen.set(Some(!lock_is_free(&probe_layout)));
        }
    });
    let error = plan.apply().unwrap_err();
    crate::test_probes::disarm("file_displaced");

    assert!(
        format!("{error:#}").contains("could not allocate tmp file"),
        "{error:#}"
    );
    assert_eq!(held.get(), Some(true), "init failed without the lock");
    assert_eq!(tree(&repo), before, "the failed init changed the files");
    for path in &occupied {
        std::fs::remove_file(path).unwrap();
    }
    let read = open_on_a_thread(&repo)
        .recv_timeout(LIMIT)
        .expect("the read after the failed init did not end");
    assert_eq!(read.unwrap(), ["reviewer"]);
    prepare_init(&repo, options("new-reviewer"))
        .unwrap()
        .apply()
        .unwrap();
    let after = manifest_text(&layout);
    assert!(after.contains("new-reviewer"), "{after}");
}

/// The input of an init of the scope `default` at the root of `repo`, as the
/// CLI gives it.
fn init_input(repo: &Utf8Path, actors: &[&str]) -> provenance_porcelain::init::InitInput {
    provenance_porcelain::init::InitInput {
        path: repo.to_string(),
        scope: Some("default".to_owned()),
        path_prefix: Some(".".to_owned()),
        disposition_actor_id: actors.iter().map(|actor| (*actor).to_owned()).collect(),
        clear_disposition_actors: false,
        ste_pdf: None,
        invocation_channel: provenance_porcelain::init::InvocationChannel::Native,
        package_manager: None,
    }
}

/// Runs `InitPort::init`, the port that the CLI and the MCP host call. Init
/// runs on a thread of the blocking pool, and a probe is local to its thread,
/// so `arm` runs on each new thread of the runtime.
fn init_through_the_port(
    input: provenance_porcelain::init::InitInput,
    arm: impl Fn() + Send + Sync + 'static,
) -> anyhow::Result<provenance_porcelain::init::InitReport> {
    use provenance_porcelain::init::InitPort as _;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .on_thread_start(arm)
        .build()?;
    runtime.block_on(crate::porcelain::SetupPort::new(crate::skills::TEST_SKILLS).init(input))
}

mod check_and_write_tests;
