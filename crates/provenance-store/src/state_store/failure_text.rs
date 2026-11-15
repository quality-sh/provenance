//! The failure text that a failed verification run stores inside the read
//! budget of its run row.
use super::read_budget::ReadBudget;
use provenance_core::{VerificationRun, VerificationRunStatus};
use provenance_macros::rule;
use std::io::{self, Write};

/// The failure text that a failed completion of the running `run` stores for `text`.
#[rule("rule_rust_verification_cuts_a_long_failure_text")]
pub fn fit_failure_text(run: &VerificationRun, text: &str) -> anyhow::Result<String> {
    // `text` when the failed row fits the read budget of a run row, else the
    // start of `text`, cut at a character boundary, with a mark that gives
    // the byte count of the stored part and of `text`.
    let mut failed = run.clone();
    failed.status = VerificationRunStatus::Failed;
    // The completion time has the digits of the start time.
    failed.completed_at = Some(run.started_at);
    failed.error = Some(String::new());
    let room = VerificationRun::read_budget().saturating_sub(serde_json::to_vec(&failed)?.len());
    if escaped_bytes(text)? <= room {
        return Ok(text.to_owned());
    }
    let mark = |kept: usize| format!("… [cut: {kept} of {} bytes]", text.len());
    // The mark of the full count is the longest mark.
    let room = room.saturating_sub(mark(text.len()).len());
    let mut kept = 0;
    let mut used = 0;
    for (at, character) in text.char_indices() {
        used += escaped_bytes(character.encode_utf8(&mut [0; 4]))?;
        if used > room {
            break;
        }
        kept = at + character.len_utf8();
    }
    Ok(format!("{}{}", &text[..kept], mark(kept)))
}

/// The bytes of `text` inside a JSON string of the run row.
fn escaped_bytes(text: &str) -> anyhow::Result<usize> {
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, text)?;
    // The two quotes of the string are not part of the text.
    Ok(counter.0 - 2)
}

struct Counter(usize);

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
