use provenance_macros::verifies;

#[verifies("rule_example", construction)]
fn sample(value: u32) -> u32 {
    value
}

fn main() {}
