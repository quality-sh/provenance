//! One `git cat-file --batch` process that gives the bytes of a file at one
//! commit per request.

use crate::stale::git::command;
use camino::Utf8Path;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::thread::JoinHandle;

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
    /// The number of digits of a commit id of the repository.
    id_length: usize,
}

/// Refuses a part of a request name with a line break. Git reads a request
/// to its line feed, and takes a carriage return before the line feed as
/// part of the line break. Another line feed would make two requests, and
/// the answer of the second would be read as the answer to the next text.
pub(super) fn refuse_line_break(part: &str) -> anyhow::Result<()> {
    crate::write_error::ensure!(
        Fault,
        !part.contains('\n') && !part.ends_with('\r'),
        "git cat-file cannot read a name with a line break"
    );
    Ok(())
}

impl Batch {
    /// Starts the process in `root`, whose path inside its work tree is
    /// `prefix` and whose commit ids have `id_length` digits.
    pub(super) fn start(root: &Utf8Path, prefix: String, id_length: usize) -> anyhow::Result<Self> {
        refuse_line_break(&prefix)?;
        let mut child = command(root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let input = child.stdin.take().expect("piped stdin");
        let output = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut stderr = child.stderr.take().expect("piped stderr");
        let diagnostic = std::thread::Builder::new()
            .name("git cat-file standard error".to_owned())
            .spawn(move || {
                let mut bytes = Vec::new();
                let _ = stderr.read_to_end(&mut bytes);
                bytes
            });
        let diagnostic = match diagnostic {
            Ok(diagnostic) => diagnostic,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        };
        Ok(Self {
            child,
            input,
            output,
            diagnostic: Some(diagnostic),
            prefix,
            id_length,
        })
    }

    /// The number of digits of a commit id of the repository.
    pub(super) const fn id_length(&self) -> usize {
        self.id_length
    }

    /// The bytes of `relative` at `commit`. `None` when Git holds no object
    /// at that name, or when the entry at that name is a gitlink. After an
    /// error the process can be in the middle of an answer, so the caller
    /// drops it.
    pub(super) fn bytes(
        &mut self,
        commit: &str,
        relative: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        // The adapter refuses a path with a line break, and `start` a prefix.
        let name = format!("{commit}:{}{relative}", self.prefix);
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
        // The answer to a name with no object repeats the name, which can
        // hold spaces, so it is read first.
        if header.ends_with(" missing") {
            return Ok(None);
        }
        // A gitlink at the path names a commit and has no text. Git answers
        // a commit that the repository does not hold with `submodule`, or
        // with `missing` in versions with no `submodule` answer, and a
        // commit that the repository holds with the commit object.
        let (kind, size) = match header.split(' ').collect::<Vec<_>>()[..] {
            [_, "submodule"] => return Ok(None),
            [_, kind, size] => size.parse::<usize>().ok().map(|size| (kind, size)),
            _ => None,
        }
        .ok_or_else(|| crate::refusal::fault!("git cat-file returned an unreadable header"))?;
        // The object, then one line break.
        let mut bytes = vec![0; size + 1];
        if self.output.read_exact(&mut bytes).is_err() {
            return Err(self.failure());
        }
        if kind == "commit" {
            return Ok(None);
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
            Ok(status) if !status.success() => crate::refusal::fault!(
                "git cat-file failed: {}",
                String::from_utf8_lossy(&diagnostic).trim()
            ),
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
