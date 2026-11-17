//! One write/read size invariant for canonical resource records.
//!
//! Supported reads limit page keys, stored row bytes, and serialized page
//! items. A write must check each limit before it publishes the record.
use provenance_core::model::projection_row::ColumnValue;
use provenance_core::model::ProjectionRow;
use provenance_macros::rule;
use serde::Serialize;

/// The bytes of one stored row of a record or a binding. The writer refuses a
/// larger row, and each read of a record or a binding takes this budget.
pub const RECORD_BYTES: usize = 65_536;
const RESOURCE_RECORD_BYTES: usize = 1_048_576 - 16_384;
const ID_BYTES: usize = 1024;

// The projection pages read with the same resource page budget.
#[cfg(feature = "host")]
const _: () = assert!(RESOURCE_RECORD_BYTES == crate::cache::read::page::RESOURCE_RECORD_BYTES);

/// The shared resource row and serialized page-item budget.
pub const fn resource_record_bytes() -> usize {
    RESOURCE_RECORD_BYTES
}

/// The row budget of a record or a binding, which the writer and each read
/// apply.
pub const fn record_bytes() -> usize {
    RECORD_BYTES
}

/// The row or payload byte accounting one supported read applies to a kind.
pub trait ReadBudget: Serialize {
    /// The stored bytes the supported reads account for this record.
    fn read_bytes(&self) -> anyhow::Result<usize>;

    /// The key that a supported page scan must return.
    fn read_id(&self) -> &str;

    /// The row budget of this kind.
    fn read_budget() -> usize {
        record_bytes()
    }

    /// The name of the field that uses the most bytes of the row, which a
    /// refusal names as the field to make shorter.
    fn longest_field(&self) -> anyhow::Result<Option<String>> {
        let serde_json::Value::Object(fields) = serde_json::to_value(self)? else {
            return Ok(None);
        };
        Ok(fields
            .iter()
            .map(|(name, value)| (value.to_string().len(), name))
            .max()
            .map(|(_, name)| name.clone()))
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
        ColumnValue::Real(_) => Err(crate::refusal::fault!(
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
            crate::write_error::ensure!(
                Fault,
                sqlite::sqlite3_open(c":memory:".as_ptr(), &raw mut connection)
                    == sqlite::SQLITE_OK,
                "cannot open SQLite for REAL byte accounting"
            );
            crate::write_error::ensure!(
                Fault,
                sqlite::sqlite3_prepare_v2(
                    connection,
                    c"SELECT CAST(? AS BLOB)".as_ptr(),
                    -1,
                    &raw mut statement,
                    std::ptr::null_mut(),
                ) == sqlite::SQLITE_OK,
                "cannot prepare SQLite REAL byte accounting"
            );
            crate::write_error::ensure!(
                Fault,
                sqlite::sqlite3_bind_double(statement, 1, real) == sqlite::SQLITE_OK,
                "cannot bind SQLite REAL for byte accounting"
            );
            crate::write_error::ensure!(
                Fault,
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
/// refuse the key or the row.
#[rule("rule_accepted_writes_stay_readable")]
pub fn ensure_within_read_budget<T: ReadBudget>(record: &T) -> anyhow::Result<()> {
    let id = record.read_id();
    if id.len() > ID_BYTES {
        return Err(too_large(id, Some("id"), id.len(), ID_BYTES));
    }
    ensure_row_within(record)
}

/// Refuses a stored row over the budget of its kind, with the record, its
/// longest field, the size of its row and the budget.
#[rule("rule_writer_refuses_a_row_over_the_record_budget")]
fn ensure_row_within<T: ReadBudget>(record: &T) -> anyhow::Result<()> {
    let (size, budget) = (record.read_bytes()?, T::read_budget());
    if size > budget {
        let field = record.longest_field()?;
        return Err(too_large(record.read_id(), field.as_deref(), size, budget));
    }
    Ok(())
}

/// Refuses the review of a restated Requirement whose row passes the record
/// budget. The row holds the text before and after the change, so the refusal
/// names the Requirement and the field that the person changed.
pub fn ensure_review_within(review: &provenance_core::RequirementReview) -> anyhow::Result<()> {
    let size = review.read_bytes()?;
    if size > RECORD_BYTES {
        let id = review.requirement_id.as_str();
        return Err(refusal(
            &format!("the review of the restated Requirement `{id}`"),
            id,
            Some(&review.field),
            size,
            RECORD_BYTES,
        ));
    }
    Ok(())
}

fn too_large(id: &str, field: Option<&str>, size: usize, budget: usize) -> anyhow::Error {
    refusal(&format!("record `{id}`"), id, field, size, budget)
}

/// A `record_too_large` refusal whose message names `subject` and the field
/// to make shorter.
fn refusal(
    subject: &str,
    id: &str,
    field: Option<&str>,
    size: usize,
    budget: usize,
) -> anyhow::Error {
    let step = field.map_or_else(
        || "make the record shorter".to_owned(),
        |field| format!("make the field `{field}` shorter"),
    );
    let message = format!("{subject} uses {size} bytes and the limit is {budget} bytes; {step}");
    crate::refusal::refuse!(
        (crate::write_error::WriteFailure::record_too_large(
            Some(id),
            field,
            size,
            budget,
            message.clone()
        )),
        "{message}"
    )
}

/// Refuses a stored line of more than `budget` bytes before it is written, for
/// a file whose reads measure each stored line.
#[cfg(feature = "host")]
pub fn ensure_line_within(line: &str, budget: usize) -> anyhow::Result<()> {
    if line.len() > budget {
        let message = format!(
            "a stored line uses {} bytes and the limit is {budget} bytes; make the line shorter",
            line.len()
        );
        return Err(crate::refusal::refuse!(
            (crate::write_error::WriteFailure::record_too_large(
                None,
                None,
                line.len(),
                budget,
                message.clone()
            )),
            "{message}"
        ));
    }
    Ok(())
}

/// Applies [`ensure_within_read_budget`] to every record of an imported or
/// landed batch.
pub fn ensure_slice_within_read_budget<T: ReadBudget>(records: &[T]) -> anyhow::Result<()> {
    for record in records {
        ensure_within_read_budget(record)?;
    }
    Ok(())
}

/// Applies [`ensure_within_read_budget`] to each record of `after` that is
/// not the same in `before`. A stored row over the budget that the write does
/// not change does not refuse the write, so a write can still make it fit.
pub fn ensure_changed_within_read_budget<T: ReadBudget + PartialEq>(
    before: &[T],
    after: &[T],
) -> anyhow::Result<()> {
    let stored: std::collections::HashMap<&str, &T> = before
        .iter()
        .map(|record| (record.read_id(), record))
        .collect();
    for record in after {
        if stored.get(record.read_id()) != Some(&record) {
            ensure_within_read_budget(record)?;
        }
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

                fn longest_field(&self) -> anyhow::Result<Option<String>> {
                    let columns = <$kind as provenance_core::model::ProjectionRow>::COLUMNS;
                    let mut longest = None;
                    for (name, value) in columns.iter().zip(self.row()?) {
                        let bytes = column_bytes(&value)?;
                        if longest.is_none_or(|(_, most)| bytes > most) {
                            longest = Some((*name, bytes));
                        }
                    }
                    Ok(longest.map(|(name, _)| name.to_owned()))
                }

                fn read_id(&self) -> &str {
                    self.id.as_str()
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

                fn read_id(&self) -> &str {
                    self.id.as_str()
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

projection_budget!(provenance_core::RequirementReview);

/// A run row has the resource budget, a second budget larger than the
/// record budget: the run reads and the evidence read of a Rule give runs, and
/// they cut their pages by page bytes, so they give a run up to that size.
impl ReadBudget for provenance_core::VerificationRun {
    fn read_bytes(&self) -> anyhow::Result<usize> {
        Ok(serde_json::to_vec(self)?.len())
    }

    fn read_id(&self) -> &str {
        self.id.as_str()
    }

    fn read_budget() -> usize {
        resource_record_bytes()
    }
}

impl ReadBudget for provenance_core::Thread {
    fn read_bytes(&self) -> anyhow::Result<usize> {
        Ok(self.scope_id.as_str().len()
            + self.id.as_str().len()
            + crate::state_store::serde_name(&self.parent.node_type)?.len()
            + self.parent.node_id.as_str().len()
            + crate::state_store::serde_name(&self.status)?.len()
            + self.created_at.to_string().len())
    }

    fn read_id(&self) -> &str {
        self.id.as_str()
    }
}

impl ReadBudget for provenance_core::Message {
    fn read_bytes(&self) -> anyhow::Result<usize> {
        Ok(serde_json::to_vec(self)?.len())
    }

    fn read_id(&self) -> &str {
        self.id.as_str()
    }
}
