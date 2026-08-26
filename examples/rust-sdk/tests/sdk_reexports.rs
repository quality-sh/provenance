#[allow(unused_imports)]
use provenance_sdk::{rule, verifies};

#[test]
fn sdk_reexports_marker_attributes() {
    let _builder = provenance_sdk::rule("fixture");
}
