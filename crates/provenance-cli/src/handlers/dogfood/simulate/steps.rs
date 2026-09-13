//! Runs one simulated first-session command and captures what it did.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct StepOutcome {
    /// The command as a user would type it, minus the binary path.
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
}

impl StepOutcome {
    pub(super) fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Spawns one child inside the sandbox and captures its streams and timing.
/// Stdin is written from a thread, so a child that fills its output pipe
/// cannot deadlock the exchange.
pub(super) fn run(
    exe: &Path,
    args: &[String],
    stdin: Option<&str>,
    sandbox: &Path,
    env: &[(String, String)],
) -> std::io::Result<StepOutcome> {
    let program = exe.file_name().map_or_else(
        || "<provenance>".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let command = format!("{program} {}", args.join(" "))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut process = Command::new(exe);
    process
        .args(args)
        .current_dir(sandbox)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        process.env(key, value);
    }

    let start = Instant::now();
    let mut child = process.spawn()?;
    let writer = child
        .stdin
        .take()
        .zip(stdin.map(str::to_owned))
        .map(|(mut pipe, input)| {
            thread::spawn(move || {
                let _ = pipe.write_all(input.as_bytes());
            })
        });
    let output = child.wait_with_output()?;
    if let Some(writer) = writer {
        let _ = writer.join();
    }

    Ok(StepOutcome {
        command,
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        elapsed: start.elapsed(),
    })
}
