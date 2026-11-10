//! The three forms of a store error.
//!
//! - `refuse!(Kind, "...")`: a refusal that the input or the state of a caller
//!   can cause. It has a declared `WriteFailure` kind. A read refuses with a
//!   `ReadFailure` value.
//! - `fault!("...")`: a broken code invariant. It reads as `write_failed` or
//!   `read_failed` (500) and keeps its text as the cause.
//! - [`state_fault`]: stored records that do not agree. It reads as
//!   `invalid_state_file` with the file and the line of the record at fault.
//!
//! The `refusals_declare_a_kind` test refuses a raw anyhow refusal outside this
//! file and `write_error.rs`.

use crate::state_store::message_ids::ShardLine;
use crate::state_store::readers::StateLineInvalid;

/// The text of a declared internal fault.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Fault(pub String);

/// The text of a declared refusal.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Reason(pub String);

/// A declared internal fault with a format text.
macro_rules! fault {
    ($($message:tt)*) => {
        ::anyhow::Error::from($crate::refusal::Fault(::std::format!($($message)*)))
    };
}
pub(crate) use fault;

/// A declared refusal: `refuse!(Kind, "...")` for a unit kind or
/// `InvalidUpdate`, or `refuse!((failure), "...")` for a built failure.
macro_rules! refuse {
    (InvalidUpdate, $($message:tt)*) => {
        $crate::write_error::WriteFailure::invalid_update(
            $crate::refusal::Reason(::std::format!($($message)*)),
        )
    };
    (($failure:expr), $($message:tt)*) => {
        $crate::write_error::SourceFailure::wrap(
            $failure,
            $crate::refusal::Reason(::std::format!($($message)*)),
        )
    };
    ($kind:ident, $($message:tt)*) => {
        $crate::write_error::SourceFailure::wrap(
            $crate::write_error::WriteFailure::$kind,
            $crate::refusal::Reason(::std::format!($($message)*)),
        )
    };
}
pub(crate) use refuse;

/// Stored records that do not agree, located at the record at fault. It reads
/// as `invalid_state_file` with that file and line.
pub fn state_fault(at: &ShardLine, message: String) -> anyhow::Error {
    StateLineInvalid {
        path: at.path.clone(),
        line: u64::try_from(at.line).unwrap_or(u64::MAX),
        message,
    }
    .into()
}
