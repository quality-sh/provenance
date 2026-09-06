use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_store::settings::{Settings, SettingsError};
use serde_json::json;

fn repository() -> (tempfile::TempDir, ProvenanceLayout) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.provenance_dir()).unwrap();
    (dir, layout)
}

#[test]
fn an_absent_settings_file_means_the_defaults() {
    let (_dir, layout) = repository();
    let policy = ReadPolicy::resolve(&Settings::load(&layout).unwrap(), None);
    assert_eq!(policy, ReadPolicy::default());
    assert!(!layout.cache_dir().exists());
}

#[test]
#[verifies("rule_freshness_flag_wins_over_the_settings_file", examples)]
fn a_settings_file_sets_the_policy_and_the_limit() {
    let (_dir, layout) = repository();
    for word in ["catch_up", "annotate_only", "refuse_stale"] {
        std::fs::write(
            layout.provenance_dir().join("settings.json"),
            json!({"read": {"freshness_policy": word, "scan_limit": 7}}).to_string(),
        )
        .unwrap();
        let settings = Settings::load(&layout).unwrap();
        let policy = ReadPolicy::resolve(&settings, None);
        assert_eq!(serde_json::to_value(policy.freshness.word()).unwrap(), word);
        assert_eq!(policy.scan_limit, 7);
        let overridden = ReadPolicy::resolve(&settings, Some(FreshnessPolicy::CatchUp));
        assert_eq!(overridden.freshness, FreshnessPolicy::CatchUp);
        assert_eq!(overridden.scan_limit, 7);
    }
}

#[test]
#[verifies("rule_invalid_read_setting_is_a_typed_refusal", examples)]
fn invalid_settings_name_the_path_key_and_allowed_values() {
    let (_dir, layout) = repository();
    let path = layout.provenance_dir().join("settings.json");
    for (input, key, allowed) in [
        (
            json!({"read": {"freshness_policy": {"catch_up": null}}}),
            "read.freshness_policy",
            "catch_up, annotate_only, refuse_stale",
        ),
        (json!({"reed": {}}), "reed", "read"),
        (
            json!({"read": {"scan_limt": 1}}),
            "read.scan_limt",
            "scan_limit",
        ),
        (
            json!({"read": {"freshness_policy": "fast"}}),
            "read.freshness_policy",
            "catch_up, annotate_only, refuse_stale",
        ),
        (
            json!({"read": {"freshness_policy": null}}),
            "read.freshness_policy",
            "catch_up, annotate_only, refuse_stale",
        ),
        (
            json!({"read": {"scan_limit": 0}}),
            "read.scan_limit",
            "whole number of at least 1",
        ),
        (
            json!({"read": {"scan_limit": -1}}),
            "read.scan_limit",
            "whole number of at least 1",
        ),
        (
            json!({"read": {"scan_limit": 1.5}}),
            "read.scan_limit",
            "whole number of at least 1",
        ),
        (
            json!({"read": {"scan_limit": "2"}}),
            "read.scan_limit",
            "whole number of at least 1",
        ),
        (json!({"read": null}), "read", "object"),
        (json!([]), "$", "object"),
    ] {
        std::fs::write(&path, input.to_string()).unwrap();
        let error = Settings::load(&layout).unwrap_err();
        assert!(
            matches!(&error, SettingsError::Invalid { path: p, key: k, .. } if p == &path && k == key),
            "{error}"
        );
        assert!(error.to_string().contains(allowed), "{error}");
        assert!(error.to_string().contains(path.as_str()), "{error}");
    }
}

#[test]
fn malformed_and_unreadable_settings_do_not_become_defaults() {
    let (_dir, layout) = repository();
    let path = layout.provenance_dir().join("settings.json");
    std::fs::write(&path, "{").unwrap();
    assert!(matches!(
        Settings::load(&layout),
        Err(SettingsError::Invalid { .. })
    ));
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(matches!(
        Settings::load(&layout),
        Err(SettingsError::Unreadable { .. })
    ));
}

#[test]
fn optional_settings_keep_the_other_default() {
    let (_dir, layout) = repository();
    for input in [
        json!({}),
        json!({"read": {}}),
        json!({"read": {"scan_limit": 9}}),
    ] {
        std::fs::write(
            layout.provenance_dir().join("settings.json"),
            input.to_string(),
        )
        .unwrap();
        let resolved = ReadPolicy::resolve(&Settings::load(&layout).unwrap(), None);
        assert_eq!(resolved.freshness, FreshnessPolicy::CatchUp);
        assert_eq!(
            resolved.scan_limit,
            input["read"]["scan_limit"].as_u64().map_or_else(
                || ReadPolicy::default().scan_limit,
                |v| usize::try_from(v).unwrap()
            )
        );
    }
}
