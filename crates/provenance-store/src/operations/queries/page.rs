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
    match serde_json::to_writer(Budget(QUERY_RESPONSE_BYTES), &response) {
        Ok(()) => Ok(()),
        Err(error) if error.is_io() => Err(ReadFailure::PageBudgetExceeded.into()),
        Err(error) => Err(error.into()),
    }?;
    Ok(answer)
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
