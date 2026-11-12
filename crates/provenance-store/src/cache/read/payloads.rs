//! Typed canonical payload reads for collaboration projection tables.

use super::page::{id_page_sql, page_ids, RESOURCE_RECORD_BYTES};
use crate::operations::reader::ReadSnapshot;
use crate::read_database::{SqlColumns, SqlValue};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::{AssertionRecord, DispositionRecord, StableId};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;

mod sealed {
    pub trait Sealed {}
    pub trait ProposalOwned: Sealed {}
}

pub trait PayloadRow: sealed::Sealed + DeserializeOwned + Serialize + Send {
    const TABLE: &'static str;
}

pub trait ProposalPayloadRow: PayloadRow + sealed::ProposalOwned {}

macro_rules! define_payload_rows {
    (
        export { $($export:tt)* }
        canonical { $(
            $variant:ident { record: $record:ty, $($rest:tt)* };
        )* }
        bindings { $($bindings:tt)* }
        internal { $($internal:tt)* }
    ) => {
        $(
            impl sealed::Sealed for $record {}
            impl PayloadRow for $record {
                const TABLE: &'static str =
                    crate::cache::ProjectionFamily::$variant.family_name();
            }
        )*
    };
}

crate::families::record_families!(define_payload_rows);

impl sealed::ProposalOwned for AssertionRecord {}
impl sealed::ProposalOwned for DispositionRecord {}
impl ProposalPayloadRow for AssertionRecord {}
impl ProposalPayloadRow for DispositionRecord {}

pub struct Payloads<'s, T> {
    snapshot: &'s ReadSnapshot,
    record: PhantomData<fn() -> T>,
}

impl ReadSnapshot {
    pub(crate) fn payloads<T: PayloadRow>(&self) -> Payloads<'_, T> {
        self.attest(T::TABLE);
        Payloads {
            snapshot: self,
            record: PhantomData,
        }
    }
}

impl<T: PayloadRow> Payloads<'_, T> {
    pub(crate) async fn record(&self, id: &str) -> anyhow::Result<Option<T>> {
        self.filtered_record(id, None).await
    }

    pub(crate) async fn ids(&self, after: &str, limit: usize) -> anyhow::Result<Vec<String>> {
        self.filtered_ids(after, limit, None).await
    }

    async fn filtered_record(
        &self,
        id: &str,
        owner: Option<&StableId>,
    ) -> anyhow::Result<Option<T>> {
        let filter = owner.map_or("", |_| " AND proposal_id = ?");
        let sql = format!(
            "SELECT length(CAST(payload AS BLOB)), \
             CASE WHEN length(CAST(payload AS BLOB)) <= ? THEN payload END \
             FROM {} WHERE scope_id = ? AND id = ?{filter}",
            crate::cache::quoted(T::TABLE)
        );
        let mut values = vec![
            SqlValue::Integer(i64::try_from(RESOURCE_RECORD_BYTES)?),
            self.snapshot.scope_value(),
            id.into(),
        ];
        if let Some(owner) = owner {
            values.push(owner.as_str().into());
        }
        let rows = self.snapshot.fetch(&sql, &values).await?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        if row.integer(0)? > i64::try_from(RESOURCE_RECORD_BYTES)? {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        Ok(Some(serde_json::from_str(
            row.optional_text(1)?
                .ok_or(ReadFailure::PageRecordTooLarge)?,
        )?))
    }

    async fn filtered_ids(
        &self,
        after: &str,
        limit: usize,
        owner: Option<&StableId>,
    ) -> anyhow::Result<Vec<String>> {
        let filter = owner.map_or("", |_| " AND proposal_id = ?");
        let sql = id_page_sql(T::TABLE, filter);
        let mut values = vec![self.snapshot.scope_value(), after.into()];
        if let Some(owner) = owner {
            values.push(owner.as_str().into());
        }
        values.push(SqlValue::Integer(i64::try_from(limit)?));
        page_ids(&self.snapshot.fetch(&sql, &values).await?)
    }
}

impl<T: ProposalPayloadRow> Payloads<'_, T> {
    pub(crate) async fn proposal_record(
        &self,
        proposal: &StableId,
        id: &str,
    ) -> anyhow::Result<Option<T>> {
        self.filtered_record(id, Some(proposal)).await
    }

    pub(crate) async fn proposal_ids(
        &self,
        proposal: &StableId,
        after: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<String>> {
        self.filtered_ids(after, limit, Some(proposal)).await
    }
}
