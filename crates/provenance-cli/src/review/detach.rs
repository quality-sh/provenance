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

/// Reuses the verified running host for the repository and scope, or starts one in the
/// background, then prints where the review page is served.
#[rule("rule_review_detach_reuses_running_host")]
pub fn run(options: &super::Options) -> anyhow::Result<()> {
    let repo = options
        .repo
        .canonicalize()
        .with_context(|| format!("cannot find repository {}", options.repo.display()))?;
    // Concurrent starts for one repository and scope wait here, so the later one reuses
    // the host the earlier one started instead of starting a second.
    let _start = start_lock(&repo, &options.scope)?;
    let (host, started) = if let Some(host) = discover(&repo, &options.scope)? {
        (host, None)
    } else {
        let (host, child, log) = start(options, &repo)?;
        (host, Some((child.id(), log)))
    };
    if manifest(&repo)?.disposition_actor_ids.is_empty() {
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
    let (pid, log) = started.map_or((None, None), |(pid, log)| (Some(pid), Some(log)));
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

/// Starts the host in its own process group with no terminal, and waits until discovery
/// verifies it. The host keeps running after this command returns.
#[rule("rule_review_detach_outlives_caller")]
fn start(
    options: &super::Options,
    repo: &Path,
) -> anyhow::Result<(DiscoveredLocalHost, Child, PathBuf)> {
    let log = log_path()?;
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
    leave_caller_process_group(&mut command);
    let mut child = command.spawn().context("cannot start the review host")?;
    let deadline = Instant::now() + START_LIMIT;
    loop {
        if let Some(status) = child.try_wait()? {
            let report = std::fs::read_to_string(&log).unwrap_or_default();
            anyhow::bail!(
                "the review host stopped with {status} before it published its location.\n{}",
                report.trim()
            );
        }
        if let Some(host) = discover(repo, &options.scope)? {
            return Ok((host, child, log));
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "the review host did not publish its location within {} seconds; see {}",
            START_LIMIT.as_secs(),
            log.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A new process group keeps the host out of the job control of the caller's shell, so the
/// end of the caller's session does not stop it.
#[cfg(unix)]
fn leave_caller_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(windows)]
fn leave_caller_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

/// An exclusive lock on one file per repository and scope in the owner-only cache, held
/// until dropped.
fn start_lock(repo: &Path, scope: &str) -> anyhow::Result<std::fs::File> {
    let mut key = repo.as_os_str().as_encoded_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(scope.as_bytes());
    let path = host_directory()?.join(format!("{:x}.lock", sha2::Sha256::digest(&key)));
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

/// An owner-only log in the per-user cache; the host's startup line is not kept.
fn log_path() -> anyhow::Result<PathBuf> {
    Ok(host_directory()?.join(format!("{}.log", uuid::Uuid::new_v4().simple())))
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

fn manifest(repo: &Path) -> anyhow::Result<provenance_core::Manifest> {
    let path = repo.join(".provenance/state/manifest.json");
    let bytes = std::fs::read(&path)
        .with_context(|| format!("cannot read the manifest {}", path.display()))?;
    serde_json::from_slice(&bytes).context("the manifest is not valid")
}
