//! Starts the review host as a background process that outlives the caller, or reuses one.

use anyhow::Context as _;
use provenance_macros::rule;
use provenance_transport::local_host::{discover, DiscoveredLocalHost};
use serde::Serialize;
use sha2::Digest as _;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// How long the command waits for the background host to publish its location.
const START_LIMIT: Duration = Duration::from_secs(20);

/// Where the review page is served, for the agent that started or found the host.
#[derive(Serialize)]
struct Location {
    endpoint: String,
    url: String,
    review_url: String,
    opened: bool,
    reused: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    log: Option<PathBuf>,
}

/// A host that this command started. Dropping it stops the host, so a failure after the
/// start leaves no host running; `keep` hands the host over to the person.
struct StartedHost {
    child: Option<Child>,
    log: PathBuf,
}

impl StartedHost {
    fn keep(mut self) -> (u32, PathBuf) {
        let child = self.child.take().expect("a started host has its process");
        (child.id(), std::mem::take(&mut self.log))
    }
}

impl Drop for StartedHost {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Reuses the verified running host for the repository and scope, or starts one in the
/// background, then prints where the review page is served.
#[rule("rule_review_detach_reuses_running_host")]
pub fn run(options: &super::Options) -> anyhow::Result<()> {
    let repo = options
        .repo
        .canonicalize()
        .with_context(|| format!("cannot find repository {}", options.repo.display()))?;
    let read_only = without_reviewer(&repo)?;
    // Concurrent starts for one repository and scope wait here, so the later one reuses
    // the host the earlier one started instead of starting a second.
    let key = host_key(&repo, &options.scope);
    let _start = start_lock(&key)?;
    let (host, started) = if let Some(host) = discover(&repo, &options.scope)? {
        ensure_same_host(&host, options)?;
        (host, None)
    } else {
        let (host, started) = start(options, &repo, &key)?;
        (host, Some(started))
    };
    if read_only {
        eprintln!("{}", crate::reviewer::review_page_warning(&options.repo));
    }
    let url = host.endpoint().join("/")?;
    let mut launch = url.clone();
    launch.set_fragment(Some(&format!(
        "launch={}",
        crate::review_link::issue_launch_code(&host)?
    )));
    let (review_url, opened) = match crate::browser::open_or_print(&launch, options.no_open) {
        crate::browser::Opening::Opened => (url.to_string(), true),
        crate::browser::Opening::Printed(_) => (launch.into(), false),
    };
    let (pid, log) = started.map_or((None, None), |started| {
        let (pid, log) = started.keep();
        (Some(pid), Some(log))
    });
    crate::output::print_json(&Location {
        endpoint: host.endpoint().as_str().trim_end_matches('/').to_owned(),
        url: url.into(),
        review_url,
        opened,
        reused: pid.is_none(),
        pid,
        log,
    })
}

/// Refuses to reuse a host that serves another repository ID or listens on another port
/// than the command asks for.
fn ensure_same_host(host: &DiscoveredLocalHost, options: &super::Options) -> anyhow::Result<()> {
    let port = host.endpoint().port().unwrap_or_default();
    anyhow::ensure!(
        host.repository_id() == options.repository_id
            && (options.port == 0 || options.port == port),
        "a review host already serves this repository and scope with --repository-id {} on \
         --port {port}. Run the command again with these values to reuse it.",
        host.repository_id()
    );
    Ok(())
}

/// Starts the host in a new session with no terminal, and waits until discovery verifies
/// it. The host keeps running after this command returns.
#[rule("rule_review_detach_outlives_caller")]
fn start(
    options: &super::Options,
    repo: &Path,
    key: &str,
) -> anyhow::Result<(DiscoveredLocalHost, StartedHost)> {
    let log = host_directory()?.join(format!("{key}.log"));
    // One log per repository and scope: a new start replaces the log of the host before.
    match std::fs::remove_file(&log) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("cannot replace the review host log {}", log.display()))
        }
    }
    let log_file = crate::owner_file::create(&log)
        .with_context(|| format!("cannot create the review host log {}", log.display()))?;
    let executable = std::env::current_exe().context("cannot find the provenance executable")?;
    let mut command = Command::new(executable);
    command
        .arg("review")
        .arg("--repo")
        .arg(repo)
        .args(["--repository-id", &options.repository_id])
        .args(["--scope", &options.scope])
        .args(["--port", &options.port.to_string()])
        .arg("--no-open")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log_file);
    let mut started = StartedHost {
        child: Some(spawn_detached(&mut command).context("cannot start the review host")?),
        log,
    };
    let deadline = Instant::now() + START_LIMIT;
    loop {
        let child = started
            .child
            .as_mut()
            .expect("a started host has its process");
        if let Some(status) = child.try_wait()? {
            let report = std::fs::read_to_string(&started.log).unwrap_or_default();
            anyhow::bail!(
                "the review host stopped with {status} before it published its location.\n{}",
                report.trim()
            );
        }
        if let Some(host) = discover(repo, &options.scope)? {
            return Ok((host, started));
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "the review host did not publish its location within {} seconds, so it was \
             stopped; see {}",
            START_LIMIT.as_secs(),
            started.log.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A new session has no controlling terminal and is out of the job control of the caller's
/// shell, so the end of the caller's session does not stop the host.
#[cfg(unix)]
fn spawn_detached(command: &mut Command) -> std::io::Result<Child> {
    use std::os::unix::process::CommandExt as _;
    // setsid is async-signal-safe, and the forked child is not a process group leader.
    unsafe {
        command.pre_exec(|| {
            rustix::process::setsid()?;
            Ok(())
        });
    }
    command.spawn()
}

/// The host leaves the caller's console and its Ctrl-C group, and leaves the caller's job
/// when the job allows it. A job that refuses breakaway keeps the host.
#[cfg(windows)]
fn spawn_detached(command: &mut Command) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt as _;
    use windows_sys::Win32::{
        Foundation::ERROR_ACCESS_DENIED,
        System::Threading::{
            CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
        },
    };
    let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    match command
        .creation_flags(flags | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
    {
        Err(error)
            if error
                .raw_os_error()
                .and_then(|code| u32::try_from(code).ok())
                == Some(ERROR_ACCESS_DENIED) =>
        {
            command.creation_flags(flags).spawn()
        }
        spawned => spawned,
    }
}

/// Names one repository and scope in the per-user cache.
fn host_key(repo: &Path, scope: &str) -> String {
    let mut key = repo.as_os_str().as_encoded_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(scope.as_bytes());
    format!("{:x}", sha2::Sha256::digest(&key))
}

/// An exclusive lock on one file per repository and scope in the owner-only cache, held
/// until dropped.
fn start_lock(key: &str) -> anyhow::Result<std::fs::File> {
    let path = host_directory()?.join(format!("{key}.lock"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| format!("cannot open the review host lock {}", path.display()))?;
    fs2::FileExt::lock_exclusive(&file)
        .with_context(|| format!("cannot lock {}", path.display()))?;
    Ok(file)
}

/// The owner-only per-user cache directory for detached host logs and locks.
fn host_directory() -> anyhow::Result<PathBuf> {
    let directory = crate::user_cache::cache_directory()
        .context("cannot find the per-user cache directory")?
        .join("provenance")
        .join("review-host");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("cannot create {}", directory.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(directory)
}

/// Whether the repository has no reviewer, so the review page is read-only.
fn without_reviewer(repo: &Path) -> anyhow::Result<bool> {
    let repo = camino::Utf8PathBuf::from_path_buf(repo.to_path_buf())
        .map_err(|path| anyhow::anyhow!("repository path {} is not UTF-8", path.display()))?;
    let store = crate::store::Store::open_required(&repo)?;
    provenance_store::layout::with_initialized_graph(store.layout(), |manifest| {
        Ok(manifest.disposition_actor_ids.is_empty())
    })
}
