//! Count the successful native envelope before an answer leaves.
use provenance_core::protocol::{
    read_failure::ReadFailure, QueryResponse, Stamped, QUERY_RESPONSE_BYTES,
};
use crate::byte_count::ByteCount;
use serde::Serialize;

pub fn checked<R: Serialize>(
    operation: &'static str,
    answer: Stamped<R>,
) -> anyhow::Result<Stamped<R>> {
    let response = QueryResponse::new(
        operation,
        Stamped {
            result: &answer.result,
            stamp: answer.stamp.clone(),
            freshness_error: answer.freshness_error.clone(),
        },
    );
    if let Err(error) = serde_json::to_writer(ByteCount::up_to(QUERY_RESPONSE_BYTES), &response) {
        if error.is_io() {
            return Err(ReadFailure::PageBudgetExceeded.into());
        }
        return Err(error.into());
    }
    Ok(answer)
}

/// The bytes by which the native envelope of `answer` passes the response
/// budget, or zero when it fits.
#[cfg(feature = "host")]
pub(super) fn excess<R: Serialize>(
    operation: &'static str,
    answer: &Stamped<R>,
) -> anyhow::Result<usize> {
    let response = QueryResponse::new(
        operation,
        Stamped {
            result: &answer.result,
            stamp: answer.stamp.clone(),
            freshness_error: answer.freshness_error.clone(),
        },
    );
    Ok(crate::byte_count::json_bytes(&response)?.saturating_sub(QUERY_RESPONSE_BYTES))
}
