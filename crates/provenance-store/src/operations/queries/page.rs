//! Count the entire successful wire envelope before a native answer leaves.
use provenance_core::protocol::{read_failure::ReadFailure, Stamp, Stamped, SDK_PROTOCOL_VERSION};
use serde::Serialize;
use std::io::{self, Write};

pub(super) const RESPONSE_BYTES: usize = 1_114_112;

pub(super) fn checked<R: Serialize>(
    operation: &str,
    answer: Stamped<R>,
) -> anyhow::Result<Stamped<R>> {
    #[derive(Serialize)]
    struct Envelope<'a, R> {
        protocol_version: u32,
        operation: &'a str,
        stamp: &'a Stamp,
        freshness_error: &'a Option<String>,
        #[serde(flatten)]
        result: &'a R,
    }
    let envelope = Envelope {
        protocol_version: SDK_PROTOCOL_VERSION,
        operation,
        stamp: &answer.stamp,
        freshness_error: &answer.freshness_error,
        result: &answer.result,
    };
    // Reserve space for the external freshness-cause field and framing.
    serde_json::to_writer(Budget(RESPONSE_BYTES - 256), &envelope)
        .map_err(|_| ReadFailure::PageBudgetExceeded)?;
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
