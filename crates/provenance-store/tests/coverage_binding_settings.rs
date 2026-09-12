//! The coverage policy setting: which severity the lifecycle binding
//! findings get, and what an invalid setting refuses.

use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::settings::{BindingFindingsSeverity, Settings, SettingsError};
use serde_json::json;

fn repository() -> (tempfile::TempDir, ProvenanceLayout) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.provenance_dir()).unwrap();
    (dir, layout)
}

fn write_settings(layout: &ProvenanceLayout, value: &serde_json::Value) {
    std::fs::write(
        layout.provenance_dir().join("settings.json"),
        value.to_string(),
    )
    .unwrap();
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn an_absent_coverage_section_selects_warning() {
    let (_dir, layout) = repository();
    for input in [
        json!({}),
        json!({"read": {"scan_limit": 9}}),
        json!({"coverage": {}}),
    ] {
        write_settings(&layout, &input);
        let settings = Settings::load(&layout).unwrap();
        assert_eq!(
            settings.coverage.binding_findings,
            BindingFindingsSeverity::Warning
        );
    }
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn the_coverage_section_selects_warning_or_error() {
    let (_dir, layout) = repository();
    write_settings(
        &layout,
        &json!({"coverage": {"binding_findings": "warning"}}),
    );
    let settings = Settings::load(&layout).unwrap();
    assert_eq!(
        settings.coverage.binding_findings,
        BindingFindingsSeverity::Warning
    );
    write_settings(&layout, &json!({"coverage": {"binding_findings": "error"}}));
    let settings = Settings::load(&layout).unwrap();
    assert_eq!(
        settings.coverage.binding_findings,
        BindingFindingsSeverity::Error
    );
}

#[test]
fn invalid_binding_findings_name_the_key_and_allowed_values() {
    let (_dir, layout) = repository();
    let path = layout.provenance_dir().join("settings.json");
    for (input, key) in [
        (
            json!({"coverage": {"binding_findings": "blocking"}}),
            "coverage.binding_findings",
        ),
        (
            json!({"coverage": {"binding_findings": null}}),
            "coverage.binding_findings",
        ),
        (
            json!({"coverage": {"binding_findings": 3}}),
            "coverage.binding_findings",
        ),
        (
            json!({"coverage": {"severty": "error"}}),
            "coverage.severty",
        ),
        (json!({"coverage": []}), "coverage"),
        (json!({"coverage": null}), "coverage"),
    ] {
        write_settings(&layout, &input);
        let error = Settings::load(&layout).unwrap_err();
        assert!(
            matches!(&error, SettingsError::Invalid { path: p, key: k, .. } if p == &path && k.as_str() == key),
            "{error}"
        );
        assert!(error.to_string().contains(path.as_str()), "{error}");
    }
}

#[test]
fn an_unknown_top_level_key_is_still_refused() {
    let (_dir, layout) = repository();
    write_settings(
        &layout,
        &json!({"coveragee": {"binding_findings": "error"}}),
    );
    let error = Settings::load(&layout).unwrap_err();
    assert!(
        matches!(&error, SettingsError::Invalid { key, .. } if key.as_str() == "coveragee"),
        "{error}"
    );
}
