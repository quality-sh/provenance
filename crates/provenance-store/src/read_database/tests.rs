//! The fault of a column that a reader cannot take names the storage class
//! that the column holds, and never the stored value.

use super::{SqlColumns, SqlValue};

fn fault<T: std::fmt::Debug>(result: anyhow::Result<T>) -> String {
    let error = result.unwrap_err();
    assert!(
        error.downcast_ref::<crate::refusal::Fault>().is_some(),
        "not a fault: {error:#}"
    );
    format!("{error:#}")
}

#[test]
fn a_column_of_another_class_names_the_class_and_not_the_value() {
    let mut row = [
        SqlValue::Null,
        SqlValue::Integer(5),
        SqlValue::Real(0.5),
        SqlValue::from("stored text"),
        SqlValue::Blob(vec![7; 100_000]),
    ];
    assert_eq!(fault(row.text(0)), "column 0 holds a NULL value, not text");
    assert_eq!(
        fault(row.text(1)),
        "column 1 holds an INTEGER value, not text"
    );
    assert_eq!(
        fault(row.optional_text(2)),
        "column 2 holds a REAL value, not text or null"
    );
    assert_eq!(
        fault(row.integer(3)),
        "column 3 holds a TEXT value, not an integer"
    );
    assert_eq!(
        fault(row.take_text(4)),
        "column 4 holds a BLOB value, not text"
    );
    assert_eq!(
        fault(row.take_optional_text(4)),
        "column 4 holds a BLOB value, not text or null"
    );
    assert_eq!(
        fault(row.integer(5)),
        "column index out of bounds: the len is 5, but the index is 5"
    );
}

#[test]
fn a_taken_text_moves_out_of_the_row() {
    let mut row = [SqlValue::from("first"), SqlValue::Null];
    assert_eq!(row.take_optional_text(1).unwrap(), None);
    assert_eq!(row.take_optional_text(0).unwrap().as_deref(), Some("first"));
    assert_eq!(row.text(0).unwrap(), "");
}
