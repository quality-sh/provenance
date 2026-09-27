//! One write/read size invariant for canonical resource records.
//!
//! Supported reads limit page keys, stored row bytes, and serialized page
//! items. A write must check each limit before it publishes the record.
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::model::projection_row::ColumnValue;
use provenance_core::model::ProjectionRow;
use provenance_macros::rule;
use serde::Serialize;

/// The shared resource row and serialized page-item budget.
pub const fn resource_record_bytes() -> usize {
    crate::cache::read::page::RESOURCE_RECORD_BYTES
}

/// The canonical row budget the shared query engine enforces, which the
/// discussion page readers also apply to messages.
pub const fn record_bytes() -> usize {
    crate::cache::read::page::RECORD_BYTES
}

/// The row or payload byte accounting one supported read applies to a kind.
pub trait ReadBudget: Serialize {
    /// The stored bytes the supported reads account for this record.
    fn read_bytes(&self) -> anyhow::Result<usize>;

    /// The key bytes that a supported page scan must return.
    fn read_id_bytes(&self) -> usize;

    /// The read budget this record kind's supported reads enforce.
    fn read_budget() -> usize {
        resource_record_bytes()
    }
}

/// The byte count `length(CAST(column AS BLOB))` reports for one column
/// value: `SQLite` renders a null as absent, an integer as its decimal text,
/// text as its UTF-8 bytes, and a real through `SQLite`'s own conversion.
fn column_bytes(value: &ColumnValue) -> anyhow::Result<usize> {
    match value {
        ColumnValue::Null => Ok(0),
        ColumnValue::Text(text) => Ok(text.len()),
        ColumnValue::Integer(integer) => Ok(integer.to_string().len()),
        ColumnValue::Real(real) => sqlite_real_bytes(*real),
    }
}

fn sqlite_real_bytes(real: f64) -> anyhow::Result<usize> {
    use libsqlite3_sys as sqlite;

    let mut connection = std::ptr::null_mut();
    let mut statement = std::ptr::null_mut();
    // Use the same REAL-to-BLOB conversion as the supported read. Formatting
    // the number in Rust, or even with SQLite printf, can give another size.
    let result = unsafe {
        (|| {
            anyhow::ensure!(
                sqlite::sqlite3_open(c":memory:".as_ptr(), &raw mut connection)
                    == sqlite::SQLITE_OK,
                "cannot open SQLite for REAL byte accounting"
            );
            anyhow::ensure!(
                sqlite::sqlite3_prepare_v2(
                    connection,
                    c"SELECT CAST(? AS BLOB)".as_ptr(),
                    -1,
                    &raw mut statement,
                    std::ptr::null_mut(),
                ) == sqlite::SQLITE_OK,
                "cannot prepare SQLite REAL byte accounting"
            );
            anyhow::ensure!(
                sqlite::sqlite3_bind_double(statement, 1, real) == sqlite::SQLITE_OK,
                "cannot bind SQLite REAL for byte accounting"
            );
            anyhow::ensure!(
                sqlite::sqlite3_step(statement) == sqlite::SQLITE_ROW,
                "cannot read SQLite REAL byte count"
            );
            Ok(usize::try_from(sqlite::sqlite3_column_bytes(statement, 0))?)
        })()
    };
    unsafe {
        if !statement.is_null() {
            sqlite::sqlite3_finalize(statement);
        }
        if !connection.is_null() {
            sqlite::sqlite3_close(connection);
        }
    }
    result
}

/// Refuses the mutation before publication when any supported read would
/// refuse the key, row, or serialized page item.
#[rule("rule_accepted_writes_stay_readable")]
pub fn ensure_within_read_budget<T: ReadBudget>(record: &T) -> anyhow::Result<()> {
    for (size, budget) in [
        (record.read_id_bytes(), 1024),
        (record.read_bytes()?, T::read_budget()),
        (serde_json::to_vec(record)?.len(), resource_record_bytes()),
    ] {
        if size > budget {
            return Err(SourceFailure::wrap(
                WriteFailure::RecordTooLarge,
                anyhow::anyhow!(
                    "record uses {size} bytes; the supported read allows at most {budget} bytes"
                ),
            ));
        }
    }
    Ok(())
}

/// Applies [`ensure_within_read_budget`] to every record of a replaced or
/// imported shard.
pub fn ensure_slice_within_read_budget<T: ReadBudget>(records: &[T]) -> anyhow::Result<()> {
    for record in records {
        ensure_within_read_budget(record)?;
    }
    Ok(())
}

macro_rules! projection_budget {
    ($($kind:ty),+ $(,)?) => {
        $(
            impl ReadBudget for $kind {
                fn read_bytes(&self) -> anyhow::Result<usize> {
                    self.row()?.iter().map(column_bytes).sum()
                }

                fn read_id_bytes(&self) -> usize {
                    self.id.as_str().len()
                }
            }
        )+
    };
}

macro_rules! payload_budget {
    ($($kind:ty),+ $(,)?) => {
        $(
            impl ReadBudget for $kind {
                fn read_bytes(&self) -> anyhow::Result<usize> {
                    Ok(serde_json::to_vec(self)?.len())
                }

                fn read_id_bytes(&self) -> usize {
                    self.id.as_str().len()
                }
            }
        )+
    };
}

macro_rules! export_budget {
    (ImplementationBindings, $record:ty) => {};
    ($variant:ident, $record:ty) => {
        projection_budget!($record);
    };
}

macro_rules! canonical_budget {
    (Threads, $record:ty) => {};
    (Messages, $record:ty) => {};
    ($variant:ident, $record:ty) => {
        payload_budget!($record);
    };
}

macro_rules! define_family_budgets {
    (
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        bindings { $($binding_variant:ident: $binding_type:ty, $binding_field:ident, $binding_path:ident, $binding_suffix:literal, $binding_table:literal, [$($binding_node:tt)*], $binding_reader:ident, [$($binding_closed:tt)*], $binding_id:ident, [$($binding_loader:tt)*], [$($binding_catalog:tt)*];)* }
        internal { $($internal:tt)* }
    ) => {
        $(export_budget!($export_variant, $export_type);)*
        $(canonical_budget!($canonical_variant, $canonical_type);)*
        $(export_budget!($binding_variant, $binding_type);)*
    };
}

crate::cache::record_families!(define_family_budgets);

impl ReadBudget for provenance_core::Thread {
    fn read_bytes(&self) -> anyhow::Result<usize> {
        Ok(self.scope_id.as_str().len()
            + self.id.as_str().len()
            + crate::state_store::serde_name(&self.parent.node_type)?.len()
            + self.parent.node_id.as_str().len()
            + crate::state_store::serde_name(&self.status)?.len()
            + self.created_at.to_string().len())
    }

    fn read_id_bytes(&self) -> usize {
        self.id.as_str().len()
    }

    fn read_budget() -> usize {
        record_bytes()
    }
}

impl ReadBudget for provenance_core::Message {
    fn read_bytes(&self) -> anyhow::Result<usize> {
        Ok(serde_json::to_vec(self)?.len())
    }

    fn read_id_bytes(&self) -> usize {
        self.id.as_str().len()
    }
    /// The discussion and document page readers enforce the smaller
    /// canonical row budget on messages, so a message write honors it.
    fn read_budget() -> usize {
        record_bytes()
    }
}
