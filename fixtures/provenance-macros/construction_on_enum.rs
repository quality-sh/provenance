use provenance_macros::verifies;

/// A type whose construction makes the violation impossible.
#[verifies("rule_example", construction)]
pub enum Limited {
    First,
    Second,
}

fn main() {}
