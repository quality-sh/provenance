//! Typed canonical payload reads for collaboration projection tables.

use super::page::{id_page_sql, RESOURCE_RECORD_BYTES};
use crate::operations::reader::ReadSnapshot;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::{AssertionRecord, DispositionRecord, StableId};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::Row;
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
        canonical { $($variant:ident: $record:ty, $field:ident, $path:ident, $suffix:literal, $table:literal, [$($node:tt)*], $reader:ident, [$($closed:tt)*], $id:ident, [$($loader:tt)*], [$($catalog:tt)*];)* }
        internal { $($internal:tt)* }
    ) => {
        $(
            impl sealed::Sealed for $record {}
            impl PayloadRow for $record {
                const TABLE: &'static str = $table;
            }
        )*
    };
}

crate::cache::record_families!(define_payload_rows);

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
        let mut query = sqlx::query(&sql)
            .bind(i64::try_from(RESOURCE_RECORD_BYTES)?)
            .bind(self.snapshot.scope().as_str())
            .bind(id);
        if let Some(owner) = owner {
            query = query.bind(owner.as_str());
        }
        let row = {
            let mut tx = self.snapshot.connection().await;
            query.fetch_optional(&mut **tx).await?
        };
        let Some(row) = row else { return Ok(None) };
        let size: i64 = row.try_get(0)?;
        if size > i64::try_from(RESOURCE_RECORD_BYTES)? {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        let payload: Option<String> = row.try_get(1)?;
        Ok(Some(serde_json::from_str(
            payload.as_deref().ok_or(ReadFailure::PageRecordTooLarge)?,
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
        let mut query = sqlx::query(&sql)
            .bind(self.snapshot.scope().as_str())
            .bind(after);
        if let Some(owner) = owner {
            query = query.bind(owner.as_str());
        }
        query = query.bind(i64::try_from(limit)?);
        let rows = {
            let mut tx = self.snapshot.connection().await;
            query.fetch_all(&mut **tx).await?
        };
        rows.iter()
            .map(|row| {
                row.try_get::<Option<String>, _>("id")?
                    .ok_or_else(|| ReadFailure::PageRecordTooLarge.into())
            })
            .collect()
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
