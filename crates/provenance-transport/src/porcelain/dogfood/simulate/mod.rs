//! Dev-build-only onboarding simulator.
//!
//! Lets the team try the whole first-run experience without onboarding a
//! real repository. Design constraints (the reason this lives under the
//! dogfood surface):
//! - The sandbox is a fresh empty directory in the system temp location;
//!   every write stays inside it. `--keep` preserves it for inspection and
//!   the default removes it.
//! - The STE dictionary is served by a loopback HTTP fixture, never the
//!   network. The asset and index caches are redirected into the sandbox.
//! - The simulator records no dogfood note and touches no note spool.

use anyhow::Context;
use camino::Utf8Path;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

mod fixture_pdf;
mod fixture_server;
mod steps;

use fixture_server::FixtureServer;
use steps::StepOutcome;

const SANDBOX_PREFIX: &str = "provenance-simulate-";
const REQUIREMENT_ID: &str = "req_simulate_onboarding";
/// One clean ASD-STE100 statement the first session writes.
const STATEMENT: &str = "The simulator records each event in one file.";

/// Runs the onboarding steps in a new sandbox and gives the readable account of the run.
pub(super) fn run(keep: bool, dir: Option<&Utf8Path>) -> anyhow::Result<String> {
    let sandbox = create_sandbox(dir)?;
    let mut guard = SandboxGuard {
        path: sandbox.clone(),
        keep,
        armed: false,
    };
    let mut out = String::new();
    let result = run_and_report(&sandbox, &mut out);
    finish_sandbox(&sandbox, keep, &mut out)?;
    guard.armed = true;
    result.map(|()| out)
}

/// Removes the sandbox when the run stops with a panic, so the default removal
/// holds outside the happy path.
struct SandboxGuard {
    path: PathBuf,
    keep: bool,
    armed: bool,
}

impl Drop for SandboxGuard {
    fn drop(&mut self) {
        if !self.armed && !self.keep {
            let _ = remove_dir_all::remove_dir_all(&self.path);
        }
    }
}

fn create_sandbox(dir: Option<&Utf8Path>) -> anyhow::Result<PathBuf> {
    let parent: PathBuf = match dir {
        Some(dir) => {
            let path: PathBuf = dir.to_owned().into();
            anyhow::ensure!(
                path.is_dir(),
                "the sandbox parent {} is not a directory",
                path.display()
            );
            path
        }
        None => std::env::temp_dir(),
    };
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let sandbox = parent.join(format!("{SANDBOX_PREFIX}{}-{suffix}", std::process::id()));
    std::fs::create_dir(&sandbox)
        .with_context(|| format!("create the sandbox directory {}", sandbox.display()))?;
    Ok(sandbox)
}

fn finish_sandbox(sandbox: &Path, keep: bool, out: &mut String) -> anyhow::Result<()> {
    if keep {
        line(out, format_args!("sandbox kept at {}", sandbox.display()));
        return Ok(());
    }
    remove_dir_all::remove_dir_all(sandbox)
        .with_context(|| format!("remove the sandbox directory {}", sandbox.display()))?;
    line(out, format_args!("sandbox removed"));
    Ok(())
}

fn run_and_report(sandbox: &Path, out: &mut String) -> anyhow::Result<()> {
    line(out, format_args!("simulated onboarding"));
    let body = fixture_pdf::dictionary_pdf();
    let server =
        FixtureServer::start(body).context("start the loopback dictionary fixture server")?;
    let exe = std::env::current_exe().context("resolve the running executable")?;
    let env = child_env(sandbox, &server);

    for step in step_arguments(sandbox) {
        let outcome = steps::run(&exe, &step.args, step.stdin.as_deref(), sandbox, &env)
            .with_context(|| format!("run the {} step", step.label))?;
        print_step(&outcome, out);
        if !outcome.succeeded() {
            line(out, format_args!("verdict: fail"));
            return Ok(());
        }
    }
    line(out, format_args!("verdict: pass"));
    Ok(())
}

/// Every child sees the loopback fixture, keeps its caches inside the
/// sandbox, and could only ever reach a spool inside the sandbox.
fn child_env(sandbox: &Path, server: &FixtureServer) -> Vec<(String, String)> {
    let inside = |name: &str| sandbox.join(name).to_string_lossy().into_owned();
    vec![
        (
            "PROVENANCE_TEST_STE100_ASSET_URL".to_owned(),
            server.asset_url(),
        ),
        ("PROVENANCE_STE100_ASSET_DIR".to_owned(), inside("assets")),
        ("PROVENANCE_STE100_INDEX_DIR".to_owned(), inside("indexes")),
        ("PROVENANCE_DOGFOOD_DIR".to_owned(), inside("dogfood")),
    ]
}

struct StepSpec {
    label: &'static str,
    args: Vec<String>,
    stdin: Option<String>,
}

fn spec(label: &'static str, args: &[&str]) -> StepSpec {
    StepSpec {
        label,
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        stdin: None,
    }
}

/// The first-session sequence a new user runs, in order.
fn step_arguments(sandbox: &Path) -> Vec<StepSpec> {
    let path = sandbox.to_string_lossy().into_owned();
    vec![
        spec(
            "init",
            &["init", "--path", path.as_str(), "--scope", "default"],
        ),
        spec("prime", &["prime", "--repo", ".", "--scope", "default"]),
        spec(
            "requirement create",
            &[
                REQUIREMENT_ID,
                "create",
                "--type",
                "requirement",
                "--repo",
                ".",
                "--scope",
                "default",
                "--statement",
                STATEMENT,
            ],
        ),
        spec(
            "statement check",
            &["check", "--repo", ".", "--statement", STATEMENT],
        ),
        spec("check", &["check", "--repo", "."]),
        spec(
            "bindings check",
            &[
                "check",
                "--bindings",
                "--repo",
                ".",
                "--path",
                ".",
                "--scope",
                "default",
            ],
        ),
    ]
}

fn print_step(outcome: &StepOutcome, out: &mut String) {
    out.push('\n');
    line(out, format_args!("$ {}", outcome.command));
    let seconds = outcome.elapsed.as_secs_f32();
    match outcome.exit_code {
        Some(code) => line(out, format_args!("exit {code}, {seconds:.2}s")),
        None => line(out, format_args!("exit signal, {seconds:.2}s")),
    }
    let stdout = outcome.stdout.trim_end();
    if !stdout.is_empty() {
        line(out, format_args!("{stdout}"));
    }
    let stderr = outcome.stderr.trim_end();
    if !stderr.is_empty() {
        line(out, format_args!("{stderr}"));
    }
}

/// Appends one line to the account of the run.
fn line(out: &mut String, text: std::fmt::Arguments<'_>) {
    use std::fmt::Write as _;
    writeln!(out, "{text}").expect("a write to a string does not fail");
}
