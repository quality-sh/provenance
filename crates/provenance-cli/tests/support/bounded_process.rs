//! Runs a process of a test with a time limit.
//!
//! A process that does not end in the limit fails the test with a report of
//! what it waits on: for each thread, its kernel wait channel and system
//! call; the files it has open; the locks on those files and the processes
//! that hold them; its child processes; and the output that it wrote. Without
//! the limit, a stopped process holds the test until the shard limit stops
//! the job, and the log shows no cause.

// Each test binary that includes this file uses only some of the helpers.
#![allow(dead_code)]

#[path = "process_report.rs"]
mod process_report;

use assert_cmd::assert::{Assert, OutputAssertExt};
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The limit for one command. The commands of the tests that use it end in
/// a few seconds on a loaded runner.
pub const LIMIT: Duration = Duration::from_secs(60);

/// How long the output pipes can stay open after the process ends. A pipe
/// that stays open longer is held by a child process that did not end.
const PIPE_CLOSE: Duration = Duration::from_secs(10);

/// A command that runs the `provenance` binary of this package.
pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

/// Runs a command to its end with the limit of [`LIMIT`].
pub trait Bounded {
    /// Runs the command and gives its output.
    fn bounded_output(&mut self) -> Output;

    /// Runs the command and gives an assertion on its output. A failed
    /// assertion shows the command.
    fn bounded_assert(&mut self) -> Assert;
}

impl Bounded for Command {
    fn bounded_output(&mut self) -> Output {
        output(self, LIMIT)
    }

    fn bounded_assert(&mut self) -> Assert {
        let description = format!("{self:?}");
        self.bounded_output()
            .assert()
            .append_context("command", description)
    }
}

/// Runs `command` with piped output. Past `limit` the test fails with the
/// report of the process and the output that it wrote.
pub fn output(command: &mut Command, limit: Duration) -> Output {
    run(command, None, limit)
}

/// Runs `command` like [`output`] with `input` on its standard input.
pub fn output_with_input(command: &mut Command, input: impl Into<Vec<u8>>) -> Output {
    run(command, Some(input.into()), LIMIT)
}

/// Runs `command` like [`output_with_input`] and gives an assertion on its
/// output. A failed assertion shows the command and the input.
pub fn assert_with_input(command: &mut Command, input: impl Into<Vec<u8>>) -> Assert {
    let description = format!("{command:?}");
    let input = input.into();
    let text = String::from_utf8_lossy(&input).into_owned();
    output_with_input(command, input)
        .assert()
        .append_context("command", description)
        .append_context("stdin", text)
}

fn run(command: &mut Command, input: Option<Vec<u8>>, limit: Duration) -> Output {
    // As with `assert_cmd`, a process that gets no input reads an empty input.
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let description = format!("{command:?}");
    // In its own process group, the process and its child processes stop together.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("cannot start {description}: {error}"));
    if let Some(input) = input {
        let mut stdin = child.stdin.take().expect("piped stdin");
        // A process that stops before it reads all of the input closes the pipe.
        std::thread::spawn(move || {
            let _ = std::io::Write::write_all(&mut stdin, &input);
        });
    }
    let stdout = drain(child.stdout.take().expect("piped stdout"));
    let stderr = drain(child.stderr.take().expect("piped stderr"));
    let Some(status) = wait_until(&mut child, Instant::now() + limit) else {
        let report = process_report::report(child.id());
        stop_group(&mut child);
        let deadline = Instant::now() + PIPE_CLOSE;
        panic!(
            "the process did not end within {limit:?}: {description}\n{report}\
             stdout:\n{}\nstderr:\n{}",
            collected(&stdout, deadline),
            collected(&stderr, deadline)
        );
    };
    // One deadline for the two pipes.
    let deadline = Instant::now() + PIPE_CLOSE;
    let closed = |pipe: mpsc::Receiver<Vec<u8>>, name: &str| {
        pipe.recv_timeout(left(deadline)).unwrap_or_else(|_| {
            panic!("{description} ended with {status}, but a child process kept its {name} open")
        })
    };
    Output {
        status,
        stdout: closed(stdout, "stdout"),
        stderr: closed(stderr, "stderr"),
    }
}

/// Waits for `child` to end. Past `limit` the test fails with the report of
/// the process; then the child is stopped, and the output of the pipes that
/// the caller did not take is added to the failure.
pub fn wait(child: &mut Child, limit: Duration, what: &str) -> ExitStatus {
    if let Some(status) = wait_until(child, Instant::now() + limit) {
        return status;
    }
    let report = process_report::report(child.id());
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let _ = child.kill();
    let _ = child.wait();
    let deadline = Instant::now() + PIPE_CLOSE;
    let text = |pipe: Option<mpsc::Receiver<Vec<u8>>>| {
        pipe.as_ref().map_or_else(
            || "(taken by the test)".to_owned(),
            |pipe| collected(pipe, deadline),
        )
    };
    panic!(
        "{what}: process {} did not end within {limit:?}\n{report}stdout:\n{}\nstderr:\n{}",
        child.id(),
        text(stdout),
        text(stderr)
    );
}

/// Stops a process that [`run`] started, with its child processes on unix.
fn stop_group(child: &mut Child) {
    #[cfg(unix)]
    {
        let group = rustix::process::Pid::from_child(child);
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn left(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn wait_until(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait().expect("read the process status") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn drain(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = send.send(bytes);
    });
    receive
}

/// The output of a stopped process, or a note when a child process still
/// holds the pipe.
fn collected(pipe: &mpsc::Receiver<Vec<u8>>, deadline: Instant) -> String {
    pipe.recv_timeout(left(deadline)).map_or_else(
        |_| "(a child process still holds the pipe)".to_owned(),
        |bytes| String::from_utf8_lossy(&bytes).into_owned(),
    )
}
