//! The four forms of a store error. Each form has one way to make it.
//!
//! - `refuse!(Kind, "...")`, or `ensure!(Kind, condition, "...")` from
//!   `write_error`: a refusal that the input or the state of a caller can
//!   cause. It has a declared `WriteFailure` kind. A read refuses with a
//!   `ReadFailure` value.
//! - [`common`]: a refusal of a common kind of the operation protocol, such
//!   as `unknown_scope` or `invalid_input` on a request field. It keeps its
//!   kind and status on a read and on a write.
//! - `fault!("...")`, or `ensure!(Fault, condition, "...")`: a broken code
//!   invariant. It reads as `write_failed` or `read_failed` (500) and keeps
//!   its text as the cause. [`caused`] is the same fault for a native error,
//!   such as an I/O error, with a context above it. `?` on a native error
//!   also gives this fault, with no context.
//! - [`state_fault`] at a known line, or `readers::record_fault` at the line
//!   of a record id: stored records that the store cannot read or that do not
//!   agree. It reads as `invalid_state_file` with the file and the line of
//!   the record at fault.
//!
//! The `refusals_declare_a_kind` test refuses a raw anyhow refusal outside this
//! file and `write_error.rs`.

use crate::state_store::message_ids::ShardLine;
use crate::state_store::readers::StateLineInvalid;
use provenance_core::protocol::failure::OperationFailure;

/// The text of a declared internal fault.
///
/// No code reads this type back from an error. It exists so that `fault!`
/// makes an error with no raw anyhow text, and so that the
/// `refusals_declare_a_kind` guard can tell a declared fault from a raw one.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Fault(pub String);

/// The text of a declared refusal, below its `SourceFailure` kind.
///
/// No code reads this type back from an error: the kind is in the
/// `SourceFailure` above it. It exists so that `refuse!` gives its text with
/// no raw anyhow text.
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

/// A refusal of a common kind. The caller gets the kind and the status of
/// `failure`; a context on the error does not change them.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
pub fn common(failure: OperationFailure) -> anyhow::Error {
    failure.into()
}

/// A declared fault that keeps the native `error` below `context`. It reads
/// as `write_failed` or `read_failed` (500), and the cause gives both texts.
/// A probe that knows the type of `error` can still give it a kind.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
pub fn caused<E, C>(error: E, context: C) -> anyhow::Error
where
    E: std::error::Error + Send + Sync + 'static,
    C: std::fmt::Display + Send + Sync + 'static,
{
    anyhow::Error::new(error).context(context)
}
