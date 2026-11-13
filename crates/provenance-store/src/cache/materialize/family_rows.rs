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
use crate::read_database::{SqlValue, Transaction};
use provenance_core::protocol::GraphNode;
use provenance_core::ScopeId;

pub(super) async fn delete_rows(
    tx: &mut Transaction,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    let sql = format!(
        "DELETE FROM {} WHERE scope_id = ?",
        quoted(family.family_name())
    );
    tx.execute(&sql, vec![SqlValue::from(scope.as_str())])
        .await?;
    if let Some(node_type) = family.node_type() {
        tx.execute(
            "DELETE FROM record_identities WHERE scope_id = ? AND node_type = ?",
            vec![
                SqlValue::from(scope.as_str()),
                SqlValue::from(node_type.as_str()),
            ],
        )
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

crate::families::record_families!(define_family_loader);

/// Writes every row of one family, for a family with no stored rows. A
/// statement binds at most `bound_values` values.
pub(super) async fn load_rows(
    tx: &mut Transaction,
    family: ProjectionFamily,
    bytes: &[u8],
    bound_values: usize,
) -> anyhow::Result<u64> {
    rows_of(family, bytes)?.write(tx, bound_values).await
}

/// Makes the stored rows of one family in `scope` equal to the rows of
/// `bytes`, and writes only the rows that differ.
pub(super) async fn replace_rows(
    tx: &mut Transaction,
    family: ProjectionFamily,
    scope: &ScopeId,
    bytes: &[u8],
    bound_values: usize,
) -> anyhow::Result<u64> {
    let rows = rows_of(family, bytes)?;
    super::row_changes::write_changes(tx, rows, family, scope, bound_values).await
}
