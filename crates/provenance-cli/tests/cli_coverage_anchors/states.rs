use super::support::{binding, Fixture, ORIGINAL};

#[test]
fn a_first_seen_site_is_new_not_unchanged() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        &fixture.source,
        format!("{ORIGINAL}\n#[verifies(\"rule_anchor\", examples)]\nfn late_arrival() {{}}\n"),
    )
    .unwrap();

    let report = fixture.rescan();

    binding(&report, "late_arrival", "new");
    binding(&report, "verifies_anchor", "unchanged");
    assert!(!report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
}

/// Without a baseline the scan has nothing to compare against, so it claims
/// nothing: every site is `new`, none carries an original coordinate.
#[test]
fn a_scan_without_a_baseline_reports_every_site_as_new() {
    let fixture = Fixture::new(ORIGINAL);

    let report = fixture.scan(None, true);

    let bindings = report["bindings"].as_array().unwrap();
    assert_ne!(bindings.as_slice(), [] as [serde_json::Value; 0]);
    for site in bindings {
        assert_eq!(site["anchor_state"], "new");
        assert!(site.get("original_line").is_none());
        assert!(site.get("original_file_path").is_none());
    }
    assert_eq!(report["warnings"].as_array().unwrap().len(), 0);
}

/// Once a scan has been reconciled, rescanning an untouched tree against it
/// changes nothing, byte for byte.
#[test]
fn consecutive_scans_of_an_untouched_tree_are_byte_identical() {
    let fixture = Fixture::new(ORIGINAL);

    let first = fixture.scan_bytes(&fixture.baseline);
    let reconciled = fixture.repo.join("reconciled.json");
    std::fs::write(&reconciled, &first).unwrap();
    let second = fixture.scan_bytes(&reconciled);

    assert_eq!(first, second);
}
