//! Count the successful native envelope before an answer leaves.
use provenance_core::protocol::{
    read_failure::ReadFailure, QueryResponse, Stamped, QUERY_RESPONSE_BYTES,
};
use serde::Serialize;
use std::io::{self, Write};

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
    if let Err(error) = serde_json::to_writer(Budget(QUERY_RESPONSE_BYTES), &response) {
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
    let mut count = Count(0);
    serde_json::to_writer(&mut count, &response)?;
    Ok(count.0.saturating_sub(QUERY_RESPONSE_BYTES))
}

#[cfg(feature = "host")]
struct Count(usize);
#[cfg(feature = "host")]
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Budget(usize);
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("page budget"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
