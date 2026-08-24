use provenance_store::state_store::{ReconciledResource, TypedResourceKind};
use std::fmt::Write as _;

use super::{AffectedRule, TypedSpecPlan};

/// Explains one plan in prose a first-time reader can follow.
pub(in crate::handlers::sdk) fn render(plan: &TypedSpecPlan) -> String {
    let mut out = String::new();
    let changed = plan
        .reconciliation
        .resources
        .iter()
        .filter(|resource| !resource.changes.is_empty())
        .collect::<Vec<_>>();
    if changed.is_empty() && plan.affected_rules.is_empty() {
        return "Nothing changes.\n".to_string();
    }
    write_changes(&mut out, &changed);
    write_rules(&mut out, &plan.affected_rules);
    out
}

fn write_changes(out: &mut String, changed: &[&ReconciledResource]) {
    if changed.is_empty() {
        return;
    }
    out.push_str("What changed\n");
    for resource in changed {
        let _ = writeln!(
            out,
            "\n  {} {} ({})",
            noun(resource.kind),
            resource.key,
            resource.id.as_str()
        );
        for change in &resource.changes {
            let _ = writeln!(out, "    {} was: {}", change.field, plain(&change.before));
            let _ = writeln!(out, "    {} now: {}", change.field, plain(&change.after));
        }
    }
}

fn write_rules(out: &mut String, rules: &[AffectedRule]) {
    if rules.is_empty() {
        return;
    }
    let _ = writeln!(out, "\nRules that deserve attention: {}", rules.len());
    for rule in rules {
        let _ = writeln!(out, "\n  {}", rule.id().as_str());
        if rule.evidence.review_required() {
            out.push_str("    review required, because the requirement it serves changed:\n");
            for reason in rule.evidence.reasons() {
                let _ = writeln!(
                    out,
                    "      {} {} changed from \"{}\" to \"{}\"",
                    reason.requirement.as_str(),
                    reason.field,
                    reason.before,
                    reason.after
                );
            }
            out.push_str("    a verification run recorded after that change clears this\n");
        } else {
            out.push_str("    no review outstanding\n");
        }
        write_sites(out, rule);
    }
}

fn write_sites(out: &mut String, rule: &AffectedRule) {
    if rule.implementations().is_empty() {
        out.push_str("    no implementation recorded\n");
    } else {
        out.push_str("    implemented at:\n");
        for site in rule.implementations() {
            let _ = writeln!(out, "      {}", site.location());
        }
    }
    if rule.verifications().is_empty() {
        out.push_str("    no verification recorded\n");
    } else {
        out.push_str("    verified at:\n");
        for site in rule.verifications() {
            let _ = writeln!(out, "      {}", site.location());
        }
    }
}

const fn noun(kind: TypedResourceKind) -> &'static str {
    match kind {
        TypedResourceKind::Source => "source",
        TypedResourceKind::Requirement => "requirement",
        TypedResourceKind::Rule => "rule",
    }
}

/// Shows a recorded value the way a reader would write it.
fn plain(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), ToString::to_string)
}
