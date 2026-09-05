//! The stored revision a read answers at, and the stamp built from it.

use super::reader::ReadContext;
use provenance_core::protocol::{Stamp, StampPolicy};
use sqlx::SqliteConnection;

/// The reader logic version the stamp carries.
///
/// It moves when reader logic changes an answer for the same rows; not for
/// a migration, and not for a fix on a live part. The pinned answers test compares
/// a frozen store's answers to a file keyed by this number, regenerated
/// only in the commit that bumps it.
///
/// History:
/// - 0: the semantics of the canonical operations before any flip.
/// - 1: one neighbour per (relation, direction, endpoint) on the served
///   operations, `prime`, and the `impact` command, so a source cited under
///   two clauses is reached once; the trace and impact `seen` sets keyed by
///   kind and id, so one id under two kinds is two records.
pub const READ_DERIVATION: u32 = 1;

/// The latest `projection_revision` row and the `projection_instance` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRevision {
    pub serial: i64,
    pub digest: String,
    pub instance_id: String,
}

impl StoredRevision {
    /// The stamp for an answer read at this revision.
    pub fn stamp(&self, policy: StampPolicy, attested: Vec<String>, live: Vec<String>) -> Stamp {
        Stamp {
            serial: self.serial,
            digest: self.digest.clone(),
            instance_id: self.instance_id.clone(),
            derivation: READ_DERIVATION,
            policy,
            attested,
            live,
        }
    }
}

/// The stamp for one finished read: the one way to build it. It consumes
/// the context and its snapshot, so nothing reads after stamping, and the
/// two word lists are derived from the handles the read took.
pub fn seal(context: ReadContext, policy: StampPolicy) -> Stamp {
    let (snapshot, live) = context.into_parts();
    let (revision, attested) = snapshot.finish();
    revision.stamp(
        policy,
        attested.into_iter().map(str::to_string).collect(),
        live.into_iter()
            .map(|word| word.word().to_string())
            .collect(),
    )
}

/// Reads the stored revision on one connection.
///
/// Inside an open transaction this is the first read, so it pins the
/// snapshot every later row comes from. `None` means the database was
/// never materialized, or predates the revision tables and so holds no
/// revision either.
pub async fn stored_revision(
    connection: &mut SqliteConnection,
) -> anyhow::Result<Option<StoredRevision>> {
    let revision: Result<Option<(i64, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_optional(&mut *connection)
    .await;
    let revision = match revision {
        Ok(revision) => revision,
        Err(error) if super::reader::is_missing_table(&error) => None,
        Err(error) => return Err(error.into()),
    };
    let Some((serial, digest)) = revision else {
        return Ok(None);
    };
    let instance_id: String = sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_one(&mut *connection)
        .await?;
    Ok(Some(StoredRevision {
        serial,
        digest,
        instance_id,
    }))
}
