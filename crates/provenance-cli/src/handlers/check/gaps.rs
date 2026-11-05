//! The gaps check category: one finding for each unfinished part of the graph and one
//! for each Rule without a Source, with the graph counts of each scope it reads.

use crate::repo_context::{initialization_hint, RepoContext};
use camino::Utf8Path;
use provenance_macros::rule;
use provenance_porcelain::check::{CategoryRun, Finding, GapsContext, ScopeCounts};
use provenance_store::cache;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::declared_scope;

/// Reads the selected scope, or every declared scope, with the catch-up read that each
/// scope opens with. A finding names its scope; the details are the gap and orphan
/// items of the earlier `gaps` and `orphans` commands.
#[rule("rule_check_scope_selects_category_records")]
pub(super) fn run(repo: &Utf8Path, selected: Option<&str>) -> Result<CategoryRun, String> {
    let hint = |error: anyhow::Error| format!("{:#}", initialization_hint(repo, error));
    let scopes = provenance_store::layout::with_initialized_graph(
        &ProvenanceLayout::new(repo),
        |manifest| {
            Ok(match selected {
                Some(selected) => vec![declared_scope(&manifest, selected)?.id.as_str().to_owned()],
                None => manifest
                    .scopes
                    .iter()
                    .map(|scope| scope.id.as_str().to_owned())
                    .collect(),
            })
        },
    )
    .map_err(hint)?;
    let mut findings = Vec::new();
    let mut counts = Vec::new();
    for scope in scopes {
        let repository = RepoContext::new(repo, scope.as_str())
            .open()
            .map_err(hint)?;
        for gap in cache::find_gaps(&repository).map_err(hint)? {
            findings.push(Finding::with_detail(
                format!(
                    "{scope}: {} `{}`: {}",
                    gap.node_type.as_str(),
                    gap.node_id,
                    gap.reason
                ),
                serde_json::to_value(&gap).expect("gap item is JSON"),
            ));
        }
        for orphan in cache::orphan_rules(&repository).map_err(hint)? {
            findings.push(Finding::with_detail(
                format!(
                    "{scope}: rule `{}` has no producing Requirement with a Source",
                    orphan.rule_id
                ),
                serde_json::to_value(&orphan).expect("orphan item is JSON"),
            ));
        }
        let health = cache::coverage_health(&repository).map_err(hint)?;
        counts.push(ScopeCounts {
            scope_id: scope,
            requirements: health.requirements.total,
            source_linked_requirements: health.source_linked_requirements,
            resolved_requirements: health.resolved_requirements,
            requirements_with_rules: health.requirements_with_rules,
            rules_total: health.rules.total,
            rules_with_complete_traceability: health.rules.with_complete_traceability,
        });
    }
    Ok(CategoryRun::Gaps {
        findings,
        context: GapsContext { scopes: counts },
    })
}
