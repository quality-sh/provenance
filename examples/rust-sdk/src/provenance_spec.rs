//! The spec declares traceability; production code stays unaware.

use provenance_sdk::{implemented_by, provenance_spec, requirement, rule, source};

provenance_spec!(pub share_links => "share-links" {
    requirement("sharing")
        .statement("Users can securely share documentation")
        .from(source("sharing-policy").document("docs/sharing-policy.md"))
        .rules([implemented_by!(
            rule("expiry").statement("Share links must expire within 30 days"),
            "src/share_links.rs",
            create_share_link
        )]),
});
