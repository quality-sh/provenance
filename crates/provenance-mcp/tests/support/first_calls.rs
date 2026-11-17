//! The first calls at the end of the server instructions, for the tests that pin the text
//! before them.

/// The words before the first calls.
pub const BEFORE_FIRST_CALLS: &str = "\n\nFor a focused start, ";

/// `instructions` up to the end of the words before the first calls, or all of
/// `instructions` when they give no first calls.
pub fn without_first_calls(instructions: &str) -> String {
    match instructions.split_once(BEFORE_FIRST_CALLS) {
        Some((guidance, _)) => format!("{guidance}{BEFORE_FIRST_CALLS}"),
        None => instructions.to_owned(),
    }
}
