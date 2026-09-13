use provenance_macros::verifies;

/// A type whose construction makes the violation impossible.
#[verifies("rule_example", construction)]
pub struct Guarded {
    inner: u32,
}

fn main() {}
