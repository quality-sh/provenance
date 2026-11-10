//! One write/read size invariant for canonical resource records.
//!
//! Supported reads limit page keys, stored row bytes, and serialized page
//! items. A write must check each limit before it publishes the record.
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::model::projection_row::ColumnValue;
use provenance_core::model::ProjectionRow;
use provenance_macros::rule;
use serde::Serialize;

const RECORD_BYTES: usize = 65_536;
const RESOURCE_RECORD_BYTES: usize = 1_048_576 - 16_384;

// The projection pages read with the same budgets.
#[cfg(feature = "host")]
const _: () = assert!(
    RECORD_BYTES == crate::cache::read::page::RECORD_BYTES
        && RESOURCE_RECORD_BYTES == crate::cache::read::page::RESOURCE_RECORD_BYTES
);

/// The shared resource row and serialized page-item budget.
pub const fn resource_record_bytes() -> usize {
    RESOURCE_RECORD_BYTES
}

/// The canonical row budget the shared query engine enforces, which the
/// discussion page readers also apply to messages.
pub const fn record_bytes() -> usize {
    RECORD_BYTES
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
///
/// Only a REAL column needs `SQLite` to count it, so only the host build,
/// which has libsqlite3, counts one. The host count stays as a defence for a
/// field that is `Eq` but serializes as a JSON float, which
/// `row_counted_kind_holds_no_float_field!` does not refuse.
pub(super) fn column_bytes(value: &ColumnValue) -> anyhow::Result<usize> {
    match value {
        ColumnValue::Null => Ok(0),
        ColumnValue::Text(text) => Ok(text.len()),
        ColumnValue::Integer(integer) => Ok(integer.to_string().len()),
        #[cfg(feature = "host")]
        ColumnValue::Real(real) => sqlite_real_bytes(*real),
        #[cfg(not(feature = "host"))]
        ColumnValue::Real(_) => Err(anyhow::anyhow!(
            "only the host build counts the bytes of a REAL column"
        )),
    }
}

/// The byte count of `real` as libsqlite3 casts it to a BLOB, the same
/// conversion as the projection.
#[cfg(feature = "host")]
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

/// Fails the build when `$kind` is not `Eq`. `projection_budget!` calls it for
/// each record kind that the write check counts column by column.
#[doc(hidden)]
#[macro_export]
macro_rules! row_counted_kind_holds_no_float_field {
    ($kind:ty) => {
        const _: () = {
            /// Refuses a row-counted kind with an `f32` or `f64` field, because
            /// such a kind is not `Eq`. A field type that is `Eq` but serializes
            /// as a JSON float, such as `serde_json::Value`, is not refused at
            /// build time; the build without `host` refuses that write with a
            /// fixed error. No current row-counted kind has such a field.
            #[provenance_macros::rule("rule_row_counted_records_hold_no_real_column")]
            #[provenance_macros::verifies(
                "rule_row_counted_records_hold_no_real_column",
                construction
            )]
            trait HoldsNoFloatField: ::core::cmp::Eq {}
            impl<T: ::core::cmp::Eq> HoldsNoFloatField for T {}
            const fn holds_no_float_field<T: HoldsNoFloatField>() {}
            holds_no_float_field::<$kind>();
        };
    };
}

macro_rules! projection_budget {
    ($($kind:ty),+ $(,)?) => {
        $(
            $crate::row_counted_kind_holds_no_float_field!($kind);

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
        export { $(
            $export_variant:ident { record: $export_type:ty, $($export_rest:tt)* };
        )* }
        canonical { $(
            $canonical_variant:ident { record: $canonical_type:ty, $($canonical_rest:tt)* };
        )* }
        bindings { $(
            $binding_variant:ident { record: $binding_type:ty, $($binding_rest:tt)* };
        )* }
        internal { $($internal:tt)* }
    ) => {
        $(export_budget!($export_variant, $export_type);)*
        $(canonical_budget!($canonical_variant, $canonical_type);)*
        $(export_budget!($binding_variant, $binding_type);)*
    };
}

crate::families::record_families!(define_family_budgets);

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
