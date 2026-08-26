use provenance_sdk::{implemented_by, rule};

fn main() {
    let _ = implemented_by!(
        rule("expiry").statement("Share links expire within 30 days"),
        "src/absent_module.rs",
        create_share_link
    );
}
