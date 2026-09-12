use std::collections::BTreeSet;

pub use provenance_core::coverage::ValidationWarning;

use crate::walker::FileScan;
use crate::{source_sites, SourceSiteRole};

pub fn validate_annotations(
    scans: &[FileScan],
    known_rule_ids: impl IntoIterator<Item = String>,
) -> Vec<ValidationWarning> {
    let known = known_rule_ids.into_iter().collect::<BTreeSet<_>>();
    let mut warnings = Vec::new();
    for scan in scans {
        for location in &scan.annotations {
            if !known.contains(&location.annotation.rule) {
                warnings.push(ValidationWarning {
                    rule_id: location.annotation.rule.clone(),
                    file_path: Some(location.file_path.clone()),
                    line: Some(location.line),
                    message: format!("unknown rule id `{}`", location.annotation.rule),
                    binding_finding: false,
                });
            }
        }
    }
    warnings
}

pub fn validate_bindings(
    scans: &[FileScan],
    known_rule_ids: impl IntoIterator<Item = String>,
) -> Vec<ValidationWarning> {
    let known = known_rule_ids.into_iter().collect::<BTreeSet<_>>();
    let mut warnings = Vec::new();
    let mut seen_rule_sites = BTreeSet::new();
    for site in source_sites(scans) {
        if !known.contains(site.rule_id()) && matches!(site, crate::SourceSite::Attribute(_)) {
            warnings.push(ValidationWarning {
                rule_id: site.rule_id().to_string(),
                file_path: Some(site.file_path().to_path_buf()),
                line: Some(site.line()),
                message: format!("unknown rule id `{}`", site.rule_id()),
                binding_finding: false,
            });
        }
        if site.role() == SourceSiteRole::Implementation
            && !seen_rule_sites.insert(site.rule_id().to_string())
        {
            warnings.push(ValidationWarning {
                rule_id: site.rule_id().to_string(),
                file_path: Some(site.file_path().to_path_buf()),
                line: Some(site.line()),
                message: format!(
                    "more than one primary implementation binding was found for #[rule(\"{}\")]",
                    site.rule_id()
                ),
                binding_finding: false,
            });
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use crate::walker::{scan_file, Language};

    use super::*;

    #[test]
    fn a_known_rule_can_be_verified_without_an_implementation_binding() {
        let scan = scan_file(
            Utf8Path::new("verification.rs"),
            Language::Rust,
            "#[verifies(\"rule_unimplemented\", examples)]\nfn verifies_declared_rule() {}",
        );

        let warnings = validate_bindings(&[scan], ["rule_unimplemented".to_string()]);

        assert!(warnings.is_empty(), "{warnings:#?}");
    }

    #[test]
    fn binding_validation_warns_for_duplicate_implementation_bindings() {
        let scan = scan_file(
            Utf8Path::new("rules.rs"),
            Language::Rust,
            "#[rule(\"rule_twice\")]\nfn first() {}\n#[rule(\"rule_twice\")]\nfn second() {}",
        );

        let warnings = validate_bindings(&[scan], ["rule_twice".to_string()]);

        let messages = warnings
            .iter()
            .map(|w| w.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages
            .iter()
            .any(|m| m.contains("more than one primary implementation binding")));
    }

    #[test]
    fn native_and_comment_implementation_bindings_are_duplicates() {
        let scan = scan_file(
            Utf8Path::new("rules.rs"),
            Language::Rust,
            "#[rule(\"rule_twice\")]\nfn native() {}\n\n\
             // @provenance rule: rule_twice\nfn portable() {}",
        );

        let warnings = validate_bindings(&[scan], ["rule_twice".to_string()]);

        assert_eq!(warnings.len(), 1, "{warnings:#?}");
        assert_eq!(warnings[0].line, Some(4));
        assert!(warnings[0]
            .message
            .contains("more than one primary implementation binding"));
    }

    #[test]
    fn two_comment_implementation_bindings_are_duplicates() {
        let scan = scan_file(
            Utf8Path::new("rules.rs"),
            Language::Rust,
            "// @provenance rule: rule_twice\nfn first() {}\n\n\
             // @provenance rule: rule_twice\nfn second() {}",
        );

        let warnings = validate_bindings(&[scan], ["rule_twice".to_string()]);

        assert_eq!(warnings.len(), 1, "{warnings:#?}");
        assert!(warnings[0]
            .message
            .contains("more than one primary implementation binding"));
    }

    #[test]
    fn comment_verification_does_not_duplicate_a_native_implementation() {
        let scan = scan_file(
            Utf8Path::new("rules.rs"),
            Language::Rust,
            "#[rule(\"rule_once\")]\nfn implementation() {}\n\n\
             // @provenance rule: rule_once\n\
             // @provenance verification: examples\nfn verifies_it() {}",
        );

        let warnings = validate_bindings(&[scan], ["rule_once".to_string()]);

        assert!(warnings.is_empty(), "{warnings:#?}");
    }

    #[test]
    fn binding_validation_still_warns_for_an_unknown_verification_target() {
        let scan = scan_file(
            Utf8Path::new("verification.rs"),
            Language::Rust,
            "#[verifies(\"rule_unknown\", examples)]\nfn verifies_unknown_rule() {}",
        );

        let warnings = validate_bindings(&[scan], ["rule_known".to_string()]);

        assert_eq!(warnings.len(), 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.message == "unknown rule id `rule_unknown`"));
    }

    #[test]
    fn coverage_validation_warns_for_unknown_rule_id_with_location() {
        let scan = scan_file(
            Utf8Path::new("unknown_rule.rs"),
            Language::Rust,
            "// heading\n// @provenance rule: rule_unknown\nfn test_rule() {}",
        );

        let warnings = validate_annotations(&[scan], ["rule_overtime".to_string()]);

        assert_eq!(warnings[0].rule_id, "rule_unknown");
        assert_eq!(
            warnings[0].file_path.as_deref(),
            Some(Utf8Path::new("unknown_rule.rs"))
        );
        assert_eq!(warnings[0].line, Some(2));
    }
}
