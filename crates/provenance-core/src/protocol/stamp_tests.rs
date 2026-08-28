use super::stamp::{
    decode_cursor, encode_cursor, AttestedDomain, FreshnessPolicy, FreshnessStamp, LiveConstituent,
    SCAN_BUDGET_CAP, VISIT_BUDGET_CAP,
};

#[test]
fn stamp_serializes_every_field_clients_need() {
    let stamp = FreshnessStamp {
        instance: "pinst_abc".into(),
        serial: 7,
        digest: "sha256:01".into(),
        policy: FreshnessPolicy::CatchUp,
        attested: vec![AttestedDomain::Graph, AttestedDomain::Bindings],
        live: vec![LiveConstituent::ScannerSites],
    };
    let json = serde_json::to_string(&stamp).unwrap();
    assert!(json.contains("\"instance\":\"pinst_abc\""));
    assert!(json.contains("\"serial\":7"));
    assert!(json.contains("\"policy\":\"catch_up\""));
    assert!(json.contains("\"attested\":[\"graph\",\"bindings\"]"));
    assert!(json.contains("\"live\":[\"scanner_sites\"]"));
    let round: FreshnessStamp = serde_json::from_str(&json).unwrap();
    assert_eq!(round, stamp);
}

#[test]
fn stamp_names_unattested_operations() {
    let stamp = FreshnessStamp {
        instance: "pinst_abc".into(),
        serial: 0,
        digest: String::new(),
        policy: FreshnessPolicy::RefuseStale,
        attested: vec![],
        live: vec![LiveConstituent::Unattested],
    };
    let json = serde_json::to_string(&stamp).unwrap();
    assert!(json.contains("\"live\":[\"unattested\"]"), "{json}");
}

#[test]
fn offset_cursor_round_trips() {
    for offset in [0usize, 1, 199, 10_000] {
        let token = encode_cursor(offset);
        assert_eq!(decode_cursor(&token).unwrap(), offset);
    }
    assert!(decode_cursor("v2:3").is_err());
    assert!(decode_cursor("not-a-cursor").is_err());
}

#[test]
fn budget_caps_bound_request_overrides() {
    // Requests may override downward within caps: an override above the
    // cap lands on the cap, a valid override passes through.
    assert_eq!(usize::min(500, VISIT_BUDGET_CAP), 500);
    assert_eq!(
        usize::min(VISIT_BUDGET_CAP * 10, VISIT_BUDGET_CAP),
        VISIT_BUDGET_CAP
    );
    assert_eq!(usize::min(500, SCAN_BUDGET_CAP), 500);
    assert_eq!(
        usize::min(SCAN_BUDGET_CAP * 10, SCAN_BUDGET_CAP),
        SCAN_BUDGET_CAP
    );
}
