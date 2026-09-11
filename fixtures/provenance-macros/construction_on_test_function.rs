use provenance_macros::verifies;

// `verifies` sits outside `#[test]` because a plain build strips `#[test]`
// items before expanding the attributes below them; the marker outside still
// names the same test function and must still be refused.
#[verifies("rule_example", construction)]
#[test]
fn sample_agrees_with_the_oracle() {
    assert_eq!(2 + 2, 4);
}

fn main() {}
