use super::support::{annotation, binding, Fixture, ORIGINAL};
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

#[test]
#[verifies("rule_anchor_resolved_before_state", examples)]
fn moved_binding_relocates_by_symbol_and_hash() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        &fixture.source,
        "#[rule(\"rule_anchor\")]\nfn decide_anchor() {}\n\nfn helper() {}\n\n\
         #[verifies(\"rule_anchor\", examples)]\nfn verifies_anchor() {}\n",
    )
    .unwrap();

    let report = fixture.rescan();
    let moved = binding(&report, "verifies_anchor", "moved");

    assert_eq!(moved["original_line"], 4);
    assert_eq!(moved["line"], 6);
    assert!(moved["anchor"]["content_hash"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(!report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
}

#[test]
#[verifies("rule_anchor_resolved_before_state", examples)]
fn edited_binding_line_is_gone_not_relocated() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        &fixture.source,
        ORIGINAL.replace("rule_anchor\", examples", "rule_anchor\", exhaustion"),
    )
    .unwrap();

    let report = fixture.rescan();
    let gone = binding(&report, "verifies_anchor", "gone");

    assert_eq!(gone["line"], 4);
    assert!(gone.get("original_line").is_none());
    assert!(report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
}

#[test]
fn untouched_binding_anchor_stays_unchanged() {
    let fixture = Fixture::new(ORIGINAL);

    let report = fixture.rescan();
    let unchanged = binding(&report, "verifies_anchor", "unchanged");

    assert_eq!(unchanged["line"], 4);
    assert!(unchanged.get("original_line").is_none());
    assert_eq!(unchanged["anchor"]["symbol"], "verifies_anchor");
}

#[test]
fn moved_annotation_relocates_with_its_function() {
    let fixture = Fixture::new(
        "// @provenance rule: rule_anchor\n// @provenance verification: examples\n\
         fn verifies_anchor() {}\n",
    );
    std::fs::write(
        &fixture.source,
        "fn helper() {}\n\n// @provenance rule: rule_anchor\n\
         // @provenance verification: examples\nfn verifies_anchor() {}\n",
    )
    .unwrap();

    let report = fixture.rescan();
    let moved = annotation(&report, "verifies_anchor", "moved");

    assert_eq!(moved["original_line"], 1);
    assert_eq!(moved["line"], 3);
}

#[test]
fn baseline_sites_outside_the_current_scan_are_not_called_gone() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        fixture.source.parent().unwrap().join("other.rs"),
        "#[verifies(\"rule_anchor\", examples)]\nfn outside_current_scan() {}\n",
    )
    .unwrap();
    let baseline = fixture.scan(None, false);
    std::fs::write(
        &fixture.baseline,
        serde_json::to_vec_pretty(&baseline).unwrap(),
    )
    .unwrap();

    let report = fixture.scan_at(&fixture.source, Some(&fixture.baseline), true);

    assert!(!report["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["item_name"] == "outside_current_scan"));
    assert!(!report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
}

#[test]
fn deleted_file_sites_are_gone_when_the_parent_is_scanned() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::remove_file(&fixture.source).unwrap();

    let report = fixture.rescan();

    binding(&report, "verifies_anchor", "gone");
    assert!(report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
}

#[test]
fn cross_file_move_relocates_and_reports_the_old_file_and_line() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        &fixture.source,
        "#[rule(\"rule_anchor\")]\nfn decide_anchor() {}\n",
    )
    .unwrap();
    std::fs::write(
        fixture.source.parent().unwrap().join("other.rs"),
        "#[verifies(\"rule_anchor\", examples)]\nfn verifies_anchor() {}\n",
    )
    .unwrap();

    let report = fixture.rescan();
    let moved = binding(&report, "verifies_anchor", "moved");

    assert_eq!(moved["original_line"], 4);
    assert!(moved["original_file_path"]
        .as_str()
        .unwrap()
        .ends_with("rules.rs"));
    assert!(moved["file_path"].as_str().unwrap().ends_with("other.rs"));
    assert_eq!(moved["line"], 1);
    assert!(!report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
}

/// The anchor turns up in two other files. The scan names both candidates and
/// picks neither; nothing is declared gone.
#[test]
fn ambiguous_cross_file_move_warns_and_names_the_candidates() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        &fixture.source,
        "#[rule(\"rule_anchor\")]\nfn decide_anchor() {}\n",
    )
    .unwrap();
    let copy = "#[verifies(\"rule_anchor\", examples)]\nfn verifies_anchor() {}\n";
    std::fs::write(fixture.source.parent().unwrap().join("other.rs"), copy).unwrap();
    std::fs::write(fixture.source.parent().unwrap().join("third.rs"), copy).unwrap();

    let report = fixture.rescan();

    let ambiguity = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|warning| warning["message"].as_str().unwrap().contains("cannot pair"))
        .expect("ambiguity warning");
    let message = ambiguity["message"].as_str().unwrap();
    assert!(message.contains("other.rs:1"));
    assert!(message.contains("third.rs:1"));
    assert!(!report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning["message"].as_str().unwrap().contains("gone")));
    assert!(!report["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["item_name"] == "verifies_anchor" && site["anchor_state"] == "gone"));
}

#[test]
fn relocation_ignores_absolute_vs_relative_path_spelling() {
    let fixture = Fixture::new(ORIGINAL);
    std::fs::write(
        &fixture.source,
        "#[rule(\"rule_anchor\")]\nfn decide_anchor() {}\n\nfn helper() {}\n\n\
         #[verifies(\"rule_anchor\", examples)]\nfn verifies_anchor() {}\n",
    )
    .unwrap();
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&fixture.repo)
        .args([
            "coverage",
            "scan",
            "--repo",
            ".",
            "--path",
            "src/../src",
            "--scope",
            "default",
            "--format",
            "json",
            "--baseline",
            fixture.baseline.to_str().unwrap(),
            "--validate-rules",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).unwrap();

    let moved = binding(&report, "verifies_anchor", "moved");
    assert_eq!(moved["line"], 6);
}
