use super::*;

#[test]
fn coverage_report_counts_annotations() {
    let report = CoverageReport::new(
        Some("abc123".into()),
        2,
        vec![AnnotationResult {
            site: SiteCore {
                rule_id: "rule_overtime".into(),
                file_path: Utf8PathBuf::from("src/payroll.rs"),
                line: 4,
                verification: None,
                anchor: None,
                anchor_state: AnchorState::Unchanged,
                original_line: None,
                original_file_path: None,
            },
            function_name: Some("pays_overtime".into()),
            coverage: "full".into(),
            confidence: 1.0,
        }],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.total_annotations, 1);
}

#[test]
fn site_core_reports_role_and_current_state() {
    let mut site = SiteCore {
        rule_id: "rule_overtime".into(),
        file_path: Utf8PathBuf::from("src/payroll.rs"),
        line: 4,
        verification: None,
        anchor: None,
        anchor_state: AnchorState::New,
        original_line: None,
        original_file_path: None,
    };

    assert_eq!(site.role(), SiteRole::Implementation);
    assert!(site.is_current());
    site.verification = Some("examples".into());
    site.anchor_state = AnchorState::Gone;
    assert_eq!(site.role(), SiteRole::Verification);
    assert!(!site.is_current());
}

#[test]
fn anchored_sites_report_role_and_current_state() {
    let site = BindingResult {
        site: SiteCore {
            rule_id: "rule_overtime".into(),
            file_path: Utf8PathBuf::from("tests/payroll.rs"),
            line: 8,
            verification: Some("examples".into()),
            anchor: None,
            anchor_state: AnchorState::Gone,
            original_line: None,
            original_file_path: None,
        },
        item_name: Some("checks_overtime".into()),
    };

    assert_eq!(AnchoredSite::role(&site), SiteRole::Verification);
    assert!(!AnchoredSite::is_current(&site));
}

#[test]
fn old_annotation_results_default_to_implementation_role() {
    let annotation: AnnotationResult = serde_json::from_str(
        r#"{
            "rule_id": "rule_overtime",
            "file_path": "src/payroll.rs",
            "line": 4,
            "function_name": "pays_overtime",
            "coverage": "full",
            "confidence": 1.0
        }"#,
    )
    .unwrap();

    assert_eq!(annotation.verification, None);
    assert!(!serde_json::to_string(&annotation)
        .unwrap()
        .contains("\"verification\""));
}

#[test]
fn legacy_coverage_baseline_round_trips_byte_for_byte() {
    let json = r#"{
  "commit": "abc123",
  "files_scanned": 1,
  "total_annotations": 1,
  "warnings": [
    {
      "rule_id": "rule_warning",
      "file_path": "src/lib.rs",
      "line": 2,
      "message": "unknown rule id `rule_warning`"
    }
  ],
  "annotations": [
    {
      "rule_id": "rule_annotation",
      "file_path": "src/lib.rs",
      "line": 4,
      "function_name": "pays_overtime",
      "coverage": "full",
      "confidence": 1.0,
      "verification": "examples",
      "anchor": {
        "symbol": "pays_overtime",
        "content_hash": "sha256:annotation"
      },
      "anchor_state": "moved",
      "original_line": 3,
      "original_file_path": "src/old.rs"
    }
  ],
  "bindings": [
    {
      "rule_id": "rule_binding",
      "file_path": "src/lib.rs",
      "line": 8,
      "item_name": "checks_overtime",
      "verification": null,
      "anchor": {
        "symbol": "checks_overtime",
        "content_hash": "sha256:binding"
      },
      "anchor_state": "unchanged",
      "original_line": 7,
      "original_file_path": "src/old.rs"
    }
  ],
  "scanned_files": [
    {
      "file_path": "src/lib.rs",
      "content": "fn pays_overtime() {}\n"
    }
  ]
}"#;

    let baseline: CoverageScan = serde_json::from_str(json).unwrap();

    assert_eq!(baseline.annotations[0].site.rule_id, "rule_annotation");
    assert_eq!(baseline.bindings[0].site.rule_id, "rule_binding");
    assert_eq!(baseline.bindings[0].site.verification, None);
    assert_eq!(serde_json::to_string_pretty(&baseline).unwrap(), json);
}
