use provenance_sdk::{provenance_spec, requirement};

provenance_spec!(share_links => "session-links" {
    requirement("sharing").statement("Users can securely share documentation"),
});

fn main() {}
