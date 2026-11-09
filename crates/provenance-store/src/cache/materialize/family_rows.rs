//! Per-family row replacement.
//!
//! A full rebuild and a catch-up pass both write one (family, scope)
//! through here, so the two paths cannot derive rows differently. The
//! eleven record families go through the one `ProjectionRow` loader; the
//! seven collaboration families keep their hand-written inserts.

use super::batch::Rows;
use super::collaboration_records;
use super::record_rows::{kind_rows_of, record_rows_of};
use crate::cache::quoted;
use crate::cache::ProjectionFamily;
use provenance_core::protocol::GraphNode;
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

pub(super) async fn delete_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {} WHERE scope_id = ?",
        quoted(family.family_name())
    )))
    .bind(scope.as_str())
    .execute(&mut **tx)
    .await?;
    if let Some(node_type) = family.node_type() {
        sqlx::query("DELETE FROM record_identities WHERE scope_id = ? AND node_type = ?")
            .bind(scope.as_str())
            .bind(node_type.as_str())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

macro_rules! family_rows_of {
    ($record:ty, record($node:ident), $bytes:ident) => {
        record_rows_of::<$record>($bytes, GraphNode::$node)
    };
    ($record:ty, kind, $bytes:ident) => {
        kind_rows_of::<$record>($bytes)
    };
    ($record:ty, payload($loader:ident), $bytes:ident) => {
        collaboration_records::$loader($bytes)
    };
}

macro_rules! define_family_loader {
    (
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        node: [$($node:tt)*],
                        reader: $reader:ident,
                        strategy: $strategy:ident,
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        catalog: [$($catalog:tt)*]
                    };
                )*
            }
        )*
    ) => {
        /// The rows of one family's records, before they are written.
        fn rows_of(family: ProjectionFamily, bytes: &[u8]) -> anyhow::Result<Rows> {
            match family {
                $($(ProjectionFamily::$variant => {
                    family_rows_of!($record, $($loader)*, bytes)
                },)*)*
            }
        }
    };
}

crate::cache::record_families!(define_family_loader);

/// Writes every row of one family, for a family with no stored rows.
pub(super) async fn load_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    bytes: &[u8],
) -> anyhow::Result<u64> {
    rows_of(family, bytes)?.write(tx).await
}

/// Makes the stored rows of one family in `scope` equal to the rows of
/// `bytes`, and writes only the rows that differ.
pub(super) async fn replace_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    scope: &ScopeId,
    bytes: &[u8],
) -> anyhow::Result<u64> {
    let rows = rows_of(family, bytes)?;
    super::row_changes::write_changes(tx, rows, family, scope).await
}
