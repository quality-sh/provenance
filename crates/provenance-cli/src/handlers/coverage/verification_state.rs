use std::collections::BTreeSet;

use crate::store::Store;
use provenance_core::ScopeId;

pub(super) struct ValidationState {
    pub rules: Vec<provenance_core::Rule>,
    pub bindings: Vec<provenance_core::VerificationBinding>,
    pub implementations: Vec<provenance_core::ImplementationBinding>,
    pub warnings: Vec<provenance_core::coverage::ValidationWarning>,
}

pub(super) fn load_validation_state(
    repo: &camino::Utf8Path,
    scope: &str,
    scans: &[provenance_scanner::FileScan],
    enabled: bool,
) -> anyhow::Result<ValidationState> {
    if !enabled {
        return Ok(ValidationState {
            rules: Vec::new(),
            bindings: Vec::new(),
            implementations: Vec::new(),
            warnings: Vec::new(),
        });
    }
    let store = Store::open(repo);
    let scope = ScopeId::new(scope)?;
    let rules = store.list_rules(&scope)?;
    let known = rules
        .iter()
        .map(|rule| rule.id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let mut warnings = provenance_scanner::validate_annotations(scans, known.iter().cloned());
    warnings.extend(provenance_scanner::validate_bindings(
        scans,
        known.iter().cloned(),
    ));
    let implementations = store.active_implementation_bindings(&scope)?;
    for site in provenance_scanner::source_sites(scans).filter(|site| {
        site.role() == provenance_scanner::SourceSiteRole::Implementation
            && implementations.iter().any(|binding| {
                binding.rule_id.as_str() == site.rule_id()
                    && !same_implementation(*site, binding, repo)
            })
    }) {
        warnings.push(provenance_scanner::ValidationWarning {
            rule_id: site.rule_id().to_string(),
            file_path: Some(site.file_path().to_path_buf()),
            line: Some(site.line()),
            message: format!(
                "more than one primary implementation binding was found for rule `{}`",
                site.rule_id()
            ),
            binding_finding: false,
        });
    }
    Ok(ValidationState {
        rules,
        bindings: store.active_verification_bindings(&scope)?,
        implementations,
        warnings,
    })
}

fn same_implementation(
    site: provenance_scanner::SourceSite<'_>,
    binding: &provenance_core::ImplementationBinding,
    repo: &camino::Utf8Path,
) -> bool {
    let file = site
        .file_path()
        .strip_prefix(repo)
        .unwrap_or_else(|_| site.file_path());
    file == binding.file && site.item_name() == Some(binding.symbol.as_str())
}
