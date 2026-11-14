//! One `git cat-file --batch` process that gives the bytes of a file at one
//! commit per request.

use crate::stale::git::command;
use camino::Utf8Path;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::thread::JoinHandle;

/// A `git cat-file` process that failed, with its standard error.
///
/// A history read reads on after an error from a text, and this failure
/// wins over that error, as when one Git process read every text of the
/// file.
#[derive(Debug, thiserror::Error)]
#[error("git cat-file failed: {0}")]
pub(in crate::review::history) struct Failed(String);

/// A running `git cat-file --batch` process in one repository.
#[derive(Debug)]
pub(super) struct Batch {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    /// Reads the standard error of the process to its end while the process
    /// runs, so that the process never waits for it to be read.
    diagnostic: Option<JoinHandle<Vec<u8>>>,
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
        let mut stderr = child.stderr.take().expect("piped stderr");
        let diagnostic = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            bytes
        });
        Ok(Self {
            child,
            input,
            output,
            diagnostic: Some(diagnostic),
            prefix,
        })
    }

    /// The bytes of `relative` at `commit`. `None` when Git holds no object
    /// at that name. After an error the process can be in the middle of an
    /// answer, so the caller drops it.
    pub(super) fn bytes(
        &mut self,
        commit: &str,
        relative: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let name = format!("{commit}:{}{relative}", self.prefix);
        // Git reads a request to its line feed, and takes a carriage return
        // before the line feed as part of the line break. Another line feed
        // would make two requests, and the answer of the second would be
        // read as the answer to the next text.
        crate::write_error::ensure!(
            Fault,
            !name.contains('\n') && !name.ends_with('\r'),
            "git cat-file cannot read a name with a line break"
        );
        let request = self
            .input
            .write_all(format!("{name}\n").as_bytes())
            .and_then(|()| self.input.flush());
        let mut header = Vec::new();
        let read = self.output.read_until(b'\n', &mut header);
        if request.is_err() || !matches!(read, Ok(1..)) || header.last() != Some(&b'\n') {
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
        if self.output.read_exact(&mut bytes).is_err() {
            return Err(self.failure());
        }
        bytes.truncate(size);
        Ok(Some(bytes))
    }

    /// The error of a process that gave no whole answer: its standard error
    /// when it failed.
    fn failure(&mut self) -> anyhow::Error {
        let _ = self.child.kill();
        let status = self.child.wait();
        let diagnostic = self
            .diagnostic
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        match status {
            Ok(status) if !status.success() => {
                Failed(String::from_utf8_lossy(&diagnostic).trim().to_owned()).into()
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
