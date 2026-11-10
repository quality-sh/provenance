// A row-counted kind with an `f64` field is not `Eq`, so the guard does not build.
#[derive(PartialEq)]
pub struct Score {
    pub value: f64,
}

provenance_store::row_counted_kind_holds_no_float_field!(Score);

fn main() {}
