use crate::wiki::model::PageKind;
use provenance_core::{
    NodeType, RequirementStatus, ResolutionInputType, ResolutionStatus, RuleSeverity, RuleStatus,
    SourceType, ThreadStatus,
};

use provenance_macros::rule;

use super::html::icon_svg;

pub(in crate::wiki::render) fn status_badge(word: &str) -> String {
    status_badge_with_label(word, &capitalize(word))
}

fn status_badge_with_label(class_word: &str, label: &str) -> String {
    let icon = match class_word {
        "approved" | "resolved" | "active" => "i-check-circle",
        _ => "i-search",
    };
    format!(
        "<span class=\"status-badge {class_word}\">{}{label}</span>",
        icon_svg(icon),
    )
}

/// A requirement marked resolved that has no decisions and no rules behind it
/// is labelled "Resolved (no decisions or rules)", never plain "Resolved".
/// The badge must not let "resolved" lie: a reader who sees the plain word
/// is entitled to assume a decision or a rule stands behind it.
#[rule("rule_requirement_badge")]
pub(in crate::wiki::render) fn requirement_status_badge(
    status: &RequirementStatus,
    decisions: usize,
    rules: usize,
) -> String {
    if matches!(status, RequirementStatus::Resolved) && decisions == 0 && rules == 0 {
        status_badge_with_label("resolved-unbacked", "Resolved (no decisions or rules)")
    } else {
        status_badge(requirement_status_word(status))
    }
}

pub(in crate::wiki::render) fn sev_chip(class_word: &str, label: &str) -> String {
    format!("<span class=\"sev {class_word}\">{label}</span>")
}

pub(in crate::wiki::render) fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

pub(in crate::wiki::render) fn counted(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{count} {noun}")
}

pub(in crate::wiki::render) fn display_name(actor: &str) -> String {
    let actor = actor.trim();
    if actor.is_empty()
        || actor.contains(char::is_whitespace)
        || actor.starts_with('@')
        || actor.contains('@')
    {
        return actor.to_string();
    }
    actor
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::wiki::render) const fn kind_class(kind: PageKind) -> &'static str {
    match kind {
        PageKind::ScopeIndex => "scope-index",
        PageKind::DomainIndex => "domain-index",
        PageKind::SearchIndex => "search-index",
        PageKind::DecisionIndex => "decision-index",
        PageKind::Unfinished => "unfinished",
        PageKind::Requirement => "requirement",
        PageKind::Resolution => "resolution",
        PageKind::Rule => "rule",
        PageKind::Source => "source",
    }
}

pub(in crate::wiki::render) const fn kind_label(kind: PageKind) -> &'static str {
    match kind {
        PageKind::ScopeIndex => "Scope",
        PageKind::DomainIndex => "Domains",
        PageKind::SearchIndex => "Search",
        PageKind::DecisionIndex => "Decisions",
        PageKind::Unfinished => "Unfinished",
        PageKind::Requirement => "Requirement",
        PageKind::Resolution => "Resolution",
        PageKind::Rule => "Rule",
        PageKind::Source => "Source",
    }
}

pub(in crate::wiki::render) const fn kind_icon(kind: PageKind) -> &'static str {
    match kind {
        PageKind::Rule => "i-shield",
        PageKind::ScopeIndex | PageKind::Source => "i-book-open",
        PageKind::SearchIndex | PageKind::Unfinished => "i-search",
        PageKind::DomainIndex | PageKind::Requirement => "i-git-branch",
        PageKind::DecisionIndex | PageKind::Resolution => "i-scale",
    }
}

pub(in crate::wiki::render) const fn requirement_status_word(
    status: &RequirementStatus,
) -> &'static str {
    match status {
        RequirementStatus::Active => "active",
        RequirementStatus::Discovery => "discovery",
        RequirementStatus::Refinement => "refinement",
        RequirementStatus::Resolved => "resolved",
    }
}

pub(in crate::wiki::render) const fn resolution_status_word(
    status: &ResolutionStatus,
) -> &'static str {
    match status {
        ResolutionStatus::Draft => "draft",
        ResolutionStatus::Review => "review",
        ResolutionStatus::Proposed => "proposed",
        ResolutionStatus::Approved => "approved",
        ResolutionStatus::Rejected => "rejected",
        ResolutionStatus::Revised => "revised",
        ResolutionStatus::Superseded => "superseded",
        ResolutionStatus::Abandoned => "abandoned",
    }
}

pub(in crate::wiki::render) const fn rule_status_word(status: &RuleStatus) -> &'static str {
    match status {
        RuleStatus::Draft => "draft",
        RuleStatus::Review => "review",
        RuleStatus::Active => "active",
        RuleStatus::Deprecated => "deprecated",
        RuleStatus::Archived => "archived",
    }
}

pub(in crate::wiki::render) const fn thread_status_word(status: &ThreadStatus) -> &'static str {
    match status {
        ThreadStatus::Active => "active",
        ThreadStatus::Resolved => "resolved",
        ThreadStatus::Archived => "archived",
    }
}

pub(in crate::wiki::render) const fn severity_word(severity: &RuleSeverity) -> &'static str {
    match severity {
        RuleSeverity::Low => "low",
        RuleSeverity::Medium => "medium",
        RuleSeverity::High => "high",
        RuleSeverity::Critical => "critical",
    }
}

pub(in crate::wiki::render) const fn source_type_label(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::Policy => "Policy",
        SourceType::Document => "Document",
        SourceType::Legislation => "Legislation",
        SourceType::CompanyAgreement => "Company agreement",
        SourceType::SystemState => "System state",
        SourceType::ExternalIntegration => "External integration",
        SourceType::DomainKnowledge => "Domain knowledge",
        SourceType::ProjectArtifact => "Project artifact",
        SourceType::Incident => "Incident",
        SourceType::ApiSpec => "API spec",
    }
}

pub(in crate::wiki::render) const fn input_type_label(
    input_type: &ResolutionInputType,
) -> &'static str {
    match input_type {
        ResolutionInputType::Regulatory => "Regulatory",
        ResolutionInputType::LegalAdvice => "Legal advice",
        ResolutionInputType::Commercial => "Commercial",
        ResolutionInputType::Benchmark => "Benchmark",
        ResolutionInputType::Technical => "Technical",
        ResolutionInputType::Incident => "Incident",
        ResolutionInputType::SourceMaterial => "Source material",
    }
}

pub(in crate::wiki::render) const fn node_type_word(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "resolution",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
        NodeType::Domain => "domain",
        NodeType::Boundary => "boundary",
    }
}

pub(in crate::wiki::render) fn format_date_iso_ms(ms: i64) -> String {
    let (year, month, day) = civil_from_days(ms.div_euclid(86_400_000));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Formats an epoch-milliseconds timestamp as a civil UTC date, mockup
/// style: `18 Apr 2026`.
pub(in crate::wiki::render) fn format_date_ms(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let month = match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    };
    format!("{day} {month} {year}")
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days` algorithm).
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub(in crate::wiki::render) fn format_confidence(confidence: f64) -> String {
    format!("{}%", (confidence * 100.0).round() as u32)
}

#[cfg(test)]
mod tests {
    use provenance_macros::verifies;

    use super::{requirement_status_badge, RequirementStatus};

    /// The status list is derived from an exhaustive match so that adding a
    /// `RequirementStatus` variant fails compilation until the new variant
    /// joins the chain, keeping the exhaustion proof below complete.
    fn all_requirement_statuses() -> Vec<RequirementStatus> {
        let mut all = vec![RequirementStatus::Active];
        while let Some(next) = match all.last().unwrap() {
            RequirementStatus::Active => Some(RequirementStatus::Discovery),
            RequirementStatus::Discovery => Some(RequirementStatus::Refinement),
            RequirementStatus::Refinement => Some(RequirementStatus::Resolved),
            RequirementStatus::Resolved => None,
        } {
            all.push(next);
        }
        all
    }

    /// The badge reads counts only as zero or nonzero, so these three
    /// representatives cover the whole count axis: the empty case, the
    /// smallest nonzero case, and a larger nonzero case.
    const COUNTS: [usize; 3] = [0, 1, 7];

    /// Pulls the visible label out of a rendered badge: everything after the
    /// icon's last `>` and before the closing `</span>`.
    fn badge_label(badge: &str) -> &str {
        let inner = badge
            .strip_suffix("</span>")
            .expect("badge is a single span element");
        inner.rsplit_once('>').expect("badge opens a tag").1
    }

    /// Independent restatement of the badge-honesty decision, used as the
    /// oracle below. Must not be implemented by calling the badge.
    ///
    /// A badge is honest when the plain word "Resolved" appears exactly when
    /// the requirement is resolved and something stands behind it, and the
    /// admission of missing backing appears exactly when it is resolved and
    /// nothing does. Every other status is left alone.
    fn badge_is_honest(
        status: &RequirementStatus,
        decisions: usize,
        rules: usize,
        label: &str,
    ) -> bool {
        let resolved = matches!(status, RequirementStatus::Resolved);
        let backed = decisions > 0 || rules > 0;
        let claims_resolved_outright = label == "Resolved";
        let admits_no_backing = label.contains("no decisions or rules");
        claims_resolved_outright == (resolved && backed)
            && admits_no_backing == (resolved && !backed)
    }

    #[test]
    #[verifies("rule_requirement_badge", exhaustion)]
    fn badge_never_lets_resolved_lie() {
        for status in all_requirement_statuses() {
            for decisions in COUNTS {
                for rules in COUNTS {
                    let badge = requirement_status_badge(&status, decisions, rules);
                    let label = badge_label(&badge);
                    assert!(
                        badge_is_honest(&status, decisions, rules, label),
                        "badge for {status:?} with {decisions} decisions and {rules} rules \
                         reads {label:?}, which the honesty property forbids"
                    );
                }
            }
        }
    }

    #[test]
    #[verifies("rule_requirement_badge", examples)]
    fn unbacked_resolved_requirement_says_so() {
        let badge = requirement_status_badge(&RequirementStatus::Resolved, 0, 0);
        assert!(badge.contains("status-badge resolved-unbacked"));
        assert_eq!(badge_label(&badge), "Resolved (no decisions or rules)");
    }
}
