//! One `git cat-file --batch` process that gives the text of a file at one
//! commit per request.

use crate::stale::git::command;
use camino::Utf8Path;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

/// A running `git cat-file --batch` process in one repository.
#[derive(Debug)]
pub(super) struct Batch {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    /// The path of the repository root inside its work tree.
    prefix: String,
}

impl Batch {
    /// Starts the process in `root`, whose path inside its work tree is
    /// `prefix`.
    pub(super) fn start(root: &Utf8Path, prefix: String) -> anyhow::Result<Self> {
        let mut child = command(root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let input = child.stdin.take().expect("piped stdin");
        let output = BufReader::new(child.stdout.take().expect("piped stdout"));
        Ok(Self {
            child,
            input,
            output,
            prefix,
        })
    }

    /// The text of `relative` at `commit`. `None` when Git holds no object
    /// at that name.
    pub(super) fn text(&mut self, commit: &str, relative: &str) -> anyhow::Result<Option<String>> {
        let name = format!("{commit}:{}{relative}", self.prefix);
        // A line break would make two requests, and the answer of the second
        // would be read as the answer to the next text.
        crate::write_error::ensure!(
            Fault,
            !name.contains(['\n', '\r']),
            "git cat-file cannot read a name with a line break"
        );
        let request = self
            .input
            .write_all(format!("{name}\n").as_bytes())
            .and_then(|()| self.input.flush());
        let mut header = Vec::new();
        let read = self.output.read_until(b'\n', &mut header)?;
        if request.is_err() || read == 0 || header.last() != Some(&b'\n') {
            return Err(self.failure());
        }
        header.pop();
        let header = std::str::from_utf8(&header)?;
        if header.ends_with(" missing") {
            return Ok(None);
        }
        let size = header
            .rsplit(' ')
            .next()
            .and_then(|size| size.parse::<usize>().ok())
            .ok_or_else(|| crate::refusal::fault!("git cat-file returned an unreadable header"))?;
        // The object, then one line break.
        let mut bytes = vec![0; size + 1];
        self.output
            .read_exact(&mut bytes)
            .map_err(|_| crate::refusal::fault!("git cat-file output ended early"))?;
        bytes.truncate(size);
        Ok(Some(String::from_utf8(bytes)?))
    }

    /// The error of a process that gave no answer: its standard error when
    /// it failed.
    fn failure(&mut self) -> anyhow::Error {
        let _ = self.child.kill();
        let mut diagnostic = String::new();
        if let Some(mut stderr) = self.child.stderr.take() {
            let _ = stderr.read_to_string(&mut diagnostic);
        }
        match self.child.wait() {
            Ok(status) if !status.success() => {
                crate::refusal::fault!("git cat-file failed: {}", diagnostic.trim())
            }
            _ => crate::refusal::fault!("git cat-file output ended early"),
        }
    }
}

/// Stops the process, which can be in the middle of an answer after an
/// error, and waits for it so that no process stays behind.
impl Drop for Batch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
