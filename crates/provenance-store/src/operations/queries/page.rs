//! Count result records and the successful wire envelope before an answer leaves.
use crate::operations::reader::RECORD_BYTES;
use provenance_core::protocol::{
    read_failure::ReadFailure, EvidenceResult, GetResult, ImpactResult, NeighborsResult,
    QueryResponse, ResolveSymbolResult, SearchResult, StaleResult, Stamped, TraceResult,
};
use serde::Serialize;
use std::io::{self, Write};

pub(super) const RESPONSE_BYTES: usize = 1_114_112;

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
    within(RESPONSE_BYTES, &response, ReadFailure::PageBudgetExceeded)?;
    Ok(answer)
}

pub(super) fn checked_result<R: PageRecords + Serialize>(
    operation: &'static str,
    answer: Stamped<R>,
) -> anyhow::Result<Stamped<R>> {
    answer.result.check_records()?;
    checked(operation, answer)
}

pub(super) trait PageRecords {
    fn check_records(&self) -> anyhow::Result<()>;
}

fn records<'a, R: Serialize + 'a>(records: impl IntoIterator<Item = &'a R>) -> anyhow::Result<()> {
    for record in records {
        within(RECORD_BYTES, record, ReadFailure::PageRecordTooLarge)?;
    }
    Ok(())
}

impl PageRecords for GetResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(self.node.iter())
    }
}

impl PageRecords for SearchResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.nodes)
    }
}

impl PageRecords for NeighborsResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.neighbors)
    }
}

impl PageRecords for TraceResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.nodes)
    }
}

impl PageRecords for ImpactResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.affected_rules)
    }
}

impl PageRecords for EvidenceResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.implementation_bindings)?;
        records(&self.verification_bindings)?;
        records(&self.verification_runs)?;
        records(self.latest_verification_run.iter())?;
        records(&self.reviews)?;
        if let Some(stale) = &self.stale {
            records(&stale.sites)?;
        }
        Ok(())
    }
}

impl PageRecords for StaleResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.sites)
    }
}

impl PageRecords for ResolveSymbolResult {
    fn check_records(&self) -> anyhow::Result<()> {
        records(&self.rules)
    }
}

fn within<R: Serialize + ?Sized>(
    limit: usize,
    value: &R,
    refusal: ReadFailure,
) -> anyhow::Result<()> {
    match serde_json::to_writer(Budget(limit), value) {
        Ok(()) => Ok(()),
        Err(error) if error.is_io() => Err(refusal.into()),
        Err(error) => Err(error.into()),
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
