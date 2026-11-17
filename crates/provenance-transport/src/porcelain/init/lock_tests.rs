//! Init and readers of the state: a second init writes in one hold of the
//! publication lock, so a read waits for init and gets the graph.
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

/// The text of AGENTS.md after a person changed it.
const PERSON_AGENTS: &str = "# Team notes\n";

/// A person changes AGENTS.md, so a second init replaces it.
fn change_agents(repo: &Utf8Path) {
    std::fs::write(repo.join("AGENTS.md"), PERSON_AGENTS).unwrap();
}

/// Opens the repository as the review follow and the commands do, on its own thread.
fn open_on_a_thread(repo: &Utf8Path) -> mpsc::Receiver<anyhow::Result<Vec<String>>> {
    let (sender, receiver) = mpsc::channel();
    let repo = repo.to_owned();
    std::thread::spawn(move || {
        let read = Repository::open(&repo, "default")
            .map_err(anyhow::Error::from)
            .and_then(|repository| repository.manifest())
            .map(|manifest| {
                manifest
                    .disposition_actor_ids
                    .into_iter()
                    .map(|actor| actor.id)
                    .collect()
            });
        let _ = sender.send(read);
    });
    receiver
}

/// An init on its own thread that stopped after it compared the first project
/// file that it replaces, before the rename. The manifest goes last.
struct StoppedInit {
    resume: mpsc::Sender<()>,
    finished: mpsc::Receiver<anyhow::Result<()>>,
}

impl StoppedInit {
    fn start(plan: InitPlan) -> Self {
        let (stopped_sender, stopped) = mpsc::channel();
        let (resume, resume_receiver) = mpsc::channel::<()>();
        let (finished_sender, finished) = mpsc::channel();
        std::thread::spawn(move || {
            let mut gate = Some((stopped_sender, resume_receiver));
            crate::test_probes::arm("project_file_compared", move || {
                if let Some((stopped, resume)) = gate.take() {
                    let _ = stopped.send(());
                    let _ = resume.recv();
                }
            });
            let _ = finished_sender.send(plan.apply().map(|_| ()));
        });
        stopped
            .recv_timeout(LIMIT)
            .expect("init did not replace a project file");
        Self { resume, finished }
    }

    fn resume(self) -> anyhow::Result<()> {
        let _ = self.resume.send(());
        self.finished
            .recv_timeout(LIMIT)
            .expect("init did not finish")
    }
}

/// While a second init replaces AGENTS.md, it holds the publication
/// lock, so a holder of the lock waits for init, and the old manifest is in
/// place. The new manifest lands when init continues, and the lock is then
/// free.
/// This test has no Rule binding: it does no read, and it checks the lock that
/// the read test depends on.
#[test]
fn second_init_holds_the_publication_lock_while_it_replaces_a_project_file() {
    let (_directory, repo, layout) = initialized();
    change_agents(&repo);
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();

    let init = StoppedInit::start(plan);
    let manifest_while_stopped = manifest_text(&layout);
    let lock_free = lock_is_free(&layout);
    init.resume().unwrap();

    assert!(
        manifest_while_stopped.contains("\"reviewer\""),
        "{manifest_while_stopped}"
    );
    assert!(
        !lock_free,
        "init replaced a project file without the publication lock"
    );
    assert!(lock_is_free(&layout), "init kept the publication lock");
    let after = manifest_text(&layout);
    assert!(after.contains("new-reviewer"), "{after}");
}

/// A repository open, the read of the review follow and of the commands,
/// starts while a second init replaces a project file. The read waits for init
/// and gets the graph from after init; it does not get an error.
#[test]
#[provenance_macros::verifies("rule_read_waits_for_repeated_init", examples)]
fn a_read_while_a_second_init_writes_gets_the_graph_after_init() {
    let (_directory, repo, layout) = initialized();
    change_agents(&repo);
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

/// The key of `path` in the map of [`tree`]: the components of the path
/// below `root`, joined with `/`. The key is the same on each platform and
/// for each separator in `path`.
fn key(root: &Utf8Path, path: &Utf8Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .components()
        .map(|component| component.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every file and folder under `root`, with the bytes of each file.
fn tree(root: &Utf8Path) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut entries = BTreeMap::new();
    let mut folders = vec![root.to_owned()];
    while let Some(folder) = folders.pop() {
        for entry in folder.read_dir_utf8().unwrap() {
            let path = entry.unwrap().into_path();
            let name = key(root, &path);
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

/// Each file and folder of `repo` that init can write. The cache folder holds
/// the locks and the derived cache, which init does not write.
fn targets(repo: &Utf8Path) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut entries = tree(repo);
    entries.retain(|name, _| !Utf8Path::new(name).starts_with(".provenance/cache"));
    entries
}

/// A second init fails after it replaced AGENTS.md, while it holds the
/// publication lock, and before it writes the manifest. The files are as
/// they were before that init, no new path stays, and the lock is free: a
/// following read and a following init work.
#[test]
#[provenance_macros::verifies("rule_init_apply_rolls_back_owned_changes", examples)]
fn a_failed_second_init_restores_the_state_and_frees_the_lock() {
    let (_directory, repo, layout) = initialized();
    // Init replaces AGENTS.md and then .gitignore. The probe removes the
    // temporary file of the .gitignore write before its rename, so init
    // fails after it replaced AGENTS.md.
    change_agents(&repo);
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();
    let before = tree(&repo);

    let held = std::rc::Rc::new(std::cell::Cell::new(None));
    let seen = std::rc::Rc::clone(&held);
    let probe_layout = layout.clone();
    let probe_repo = repo.clone();
    crate::test_probes::arm("project_file_compared", move || {
        if seen.get().is_none() {
            seen.set(Some(!lock_is_free(&probe_layout)));
        }
        remove_temporary_files(&probe_repo, ".gitignore");
    });
    let error = plan.apply().unwrap_err();
    crate::test_probes::disarm("project_file_compared");

    let gitignore = repo.join(".gitignore");
    assert!(
        format!("{error:#}").starts_with(&format!(
            "failed to replace {}: ",
            gitignore.as_std_path().display()
        )),
        "{error:#}"
    );
    assert_eq!(held.get(), Some(true), "init failed without the lock");
    assert_eq!(tree(&repo), before, "the failed init changed the files");
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

/// An initialized repository in which the person changed AGENTS.md, so that
/// a second init replaces it and no other project file.
fn repository_with_changed_agents() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    change_agents(&repo);
    (directory, repo)
}

/// The reviewers in the manifest of the scope `default` of `repo`.
fn reviewers(repo: &Utf8Path) -> anyhow::Result<Vec<String>> {
    let manifest = Repository::open(repo, "default")?.manifest()?;
    Ok(manifest
        .disposition_actor_ids
        .into_iter()
        .map(|actor| actor.id)
        .collect())
}

/// The paths of `repo` with `.provenance-` in the name: the temporary files
/// of init.
fn temporary_files(repo: &Utf8Path) -> Vec<String> {
    tree(repo)
        .into_keys()
        .filter(|name| name.contains(".provenance-"))
        .collect()
}

/// Removes the temporary file of the replacement of `file_name` in `folder`.
/// A probe at `project_file_compared` calls it, so that the rename of that
/// file fails after the compare.
pub(super) fn remove_temporary_files(folder: &Utf8Path, file_name: &str) {
    let prefix = format!(".{file_name}.provenance-");
    for entry in folder.read_dir_utf8().unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().starts_with(&prefix) && entry.path().extension() == Some("tmp") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
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
mod edit_during_replacement_tests;
#[cfg(unix)]
mod folder_sync_tests;
mod moved_state_tests;
mod removal_tests;
mod replacement_stop_tests;
mod stop_tests;
