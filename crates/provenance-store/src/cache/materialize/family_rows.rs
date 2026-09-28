//! Per-family row replacement.
//!
//! A full rebuild and a catch-up pass both write one (family, scope)
//! through here, so the two paths cannot derive rows differently. The
//! eleven record families go through the one `ProjectionRow` loader; the
//! seven collaboration families keep their hand-written inserts.

use super::collaboration_records;
use super::record_rows::{load_kind, load_record};
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
    sqlx::query(&format!(
        "DELETE FROM {} WHERE scope_id = ?",
        quoted(family.family_name())
    ))
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

macro_rules! load_family {
    ($record:ty, record($node:ident), $tx:ident, $bytes:ident) => {
        load_record::<$record>($tx, $bytes, GraphNode::$node).await
    };
    ($record:ty, kind, $tx:ident, $bytes:ident) => {
        load_kind::<$record>($tx, $bytes).await
    };
    ($record:ty, payload($loader:ident), $tx:ident, $bytes:ident) => {
        collaboration_records::$loader($tx, $bytes).await
    };
    ($record:ty, journal, $tx:ident, $bytes:ident) => {
        crate::review::cache::load_rows($tx, $bytes).await
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
                        closed: [$($closed:tt)*],
                        strategy: $strategy:ident,
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        catalog: [$($catalog:tt)*]
                    };
                )*
            }
        )*
    ) => {
        pub(super) async fn load_rows(
            tx: &mut Transaction<'_, Sqlite>,
            family: ProjectionFamily,
            bytes: &[u8],
        ) -> anyhow::Result<u64> {
            match family {
                $($(ProjectionFamily::$variant => {
                    load_family!($record, $($loader)*, tx, bytes)
                },)*)*
            }
        }
    };
}

crate::cache::record_families!(define_family_loader);
