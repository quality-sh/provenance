//! The failure text that a failed verification run stores inside the read
//! budget of its run row.
use super::read_budget::ReadBudget;
use crate::byte_count::json_bytes;
use provenance_core::VerificationRun;

/// The failure text that the completed row `failed` stores for `text`.
///
/// The stored line of the row also carries `carried` bytes of members that
/// the run type does not know. The text is `text` when the line fits the read
/// budget of a run row. Else it is the start of `text`, cut at a character
/// boundary, with a mark that gives the byte count of the stored part and of
/// `text`.
pub(super) fn fit_failure_text(
    failed: &VerificationRun,
    carried: usize,
    text: &str,
) -> anyhow::Result<String> {
    let mut empty = failed.clone();
    empty.error = Some(String::new());
    let used = json_bytes(&empty)?.saturating_add(carried);
    let room = VerificationRun::read_budget().saturating_sub(used);
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
    // The two quotes of the string are not part of the text.
    Ok(json_bytes(text)? - 2)
}
