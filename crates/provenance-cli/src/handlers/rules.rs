use super::common::stable_ids;
use super::references::{self, RuleList};
use crate::cli::policy::RulesCommand;
use crate::output;
use provenance_core::{Rule, RuleSeverity, RuleStatus, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateRuleInput, StateStore},
};

/// How much of a statement a list line carries.
///
/// A list is for finding the rule you meant, not for reading it; `show` is
/// where the whole statement lives.
const STATEMENT_PREVIEW_CHARS: usize = 100;

/// Where a rule was read out of.
///
/// Kept as one thing rather than two loose columns because that is what it
/// is: a citation. A rule with neither half has no `source` at all.
#[derive(serde::Serialize)]
struct SourceBinding {
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<String>,
}

impl SourceBinding {
    fn of(rule: &Rule) -> Option<Self> {
        if rule.source_document.is_none() && rule.source_section.is_none() {
            return None;
        }
        Some(Self {
            document: rule.source_document.clone(),
            section: rule.source_section.clone(),
        })
    }
}

/// One line of a rule list: enough to pick a rule out, not enough to read it.
#[derive(serde::Serialize)]
struct RuleSummary {
    id: String,
    status: RuleStatus,
    severity: RuleSeverity,
    statement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<SourceBinding>,
}

impl RuleSummary {
    fn of(rule: &Rule) -> Self {
        Self {
            id: rule.id.as_str().to_string(),
            status: rule.status.clone(),
            severity: rule.severity.clone(),
            statement: statement_preview(&rule.statement),
            source: SourceBinding::of(rule),
        }
    }
}

/// Cuts a statement to a readable length, on a character boundary, marking
/// the cut so a reader knows there is more to fetch with `show`.
fn statement_preview(statement: &str) -> String {
    match statement.char_indices().nth(STATEMENT_PREVIEW_CHARS) {
        None => statement.to_string(),
        Some((cut, _)) => format!("{}...", statement[..cut].trim_end()),
    }
}

pub(super) fn handle(command: RulesCommand) -> anyhow::Result<()> {
    match command {
        RulesCommand::Create {
            repo,
            scope,
            id,
            name,
            description,
            requirement_id,
            resolution_id,
            statement,
            status,
            severity,
            source_document,
            source_section,
            origin_thread,
            origin_message,
            format,
        } => {
            let rule =
                StateStore::new(ProvenanceLayout::new(repo)).create_rule(CreateRuleInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    name,
                    description,
                    requirement_ids: stable_ids(requirement_id)?,
                    resolution_ids: stable_ids(resolution_id)?,
                    statement,
                    status: RuleStatus::parse(&status)?,
                    severity: RuleSeverity::parse(&severity)?,
                    source_document,
                    source_section,
                    origin_thread: origin_thread.map(StableId::new).transpose()?,
                    origin_message: origin_message.map(StableId::new).transpose()?,
                })?;
            output::print(format, &rule)?;
        }
        RulesCommand::Requirement { command } => {
            references::rule_list(RuleList::Requirement, command)?;
        }
        RulesCommand::Resolution { command } => {
            references::rule_list(RuleList::Resolution, command)?;
        }
        RulesCommand::List {
            repo,
            scope,
            format,
        } => {
            let rules = StateStore::new(ProvenanceLayout::new(repo))
                .list_rules(&ScopeId::new(scope)?)?
                .iter()
                .map(RuleSummary::of)
                .collect::<Vec<_>>();
            output::print(format, &rules)?;
        }
        RulesCommand::Show {
            repo,
            scope,
            id,
            format,
        } => {
            let id = StableId::new(id)?;
            let rule = StateStore::new(ProvenanceLayout::new(repo))
                .list_rules(&ScopeId::new(scope)?)?
                .into_iter()
                .find(|rule| rule.id == id)
                .ok_or_else(|| anyhow::anyhow!("rule `{}` not found in scope", id.as_str()))?;
            output::print(format, &rule)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{statement_preview, RuleSummary, SourceBinding, STATEMENT_PREVIEW_CHARS};
    use provenance_core::{Rule, RuleSeverity, RuleStatus, SchemaVersion, ScopeId, StableId};

    fn rule(statement: &str) -> Rule {
        Rule {
            schema_version: SchemaVersion(1),
            scope_id: ScopeId::new("default").unwrap(),
            id: StableId::new("rule_overtime").unwrap(),
            declared_by: None,
            declaration_address: None,
            retired: false,
            name: None,
            description: None,
            statement: statement.to_string(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            requirement_ids: Vec::new(),
            resolution_ids: Vec::new(),
            origin_thread: None,
            origin_message: None,
        }
    }

    #[test]
    fn a_short_statement_is_carried_whole() {
        assert_eq!(
            statement_preview("Pay overtime after the threshold"),
            "Pay overtime after the threshold"
        );
    }

    #[test]
    fn a_long_statement_is_cut_and_marked() {
        let statement = "x".repeat(STATEMENT_PREVIEW_CHARS + 40);

        let preview = statement_preview(&statement);

        assert_eq!(preview.len(), STATEMENT_PREVIEW_CHARS + 3);
        assert!(preview.ends_with("..."));
    }

    /// Cutting by bytes would split a multi-byte character and panic.
    #[test]
    fn a_statement_of_multi_byte_characters_is_cut_on_a_character_boundary() {
        let statement = "é".repeat(STATEMENT_PREVIEW_CHARS + 10);

        let preview = statement_preview(&statement);

        assert_eq!(preview.chars().count(), STATEMENT_PREVIEW_CHARS + 3);
    }

    #[test]
    fn a_rule_citing_nothing_has_no_source_binding() {
        assert!(SourceBinding::of(&rule("Pay overtime")).is_none());
    }

    #[test]
    fn a_rule_citing_only_a_section_still_has_a_source_binding() {
        let cited = Rule {
            source_section: Some("Clause 4.2".to_string()),
            ..rule("Pay overtime")
        };

        let binding = SourceBinding::of(&cited).unwrap();

        assert_eq!(binding.document, None);
        assert_eq!(binding.section.as_deref(), Some("Clause 4.2"));
    }

    #[test]
    fn a_summary_carries_the_fields_a_list_line_shows() {
        let cited = Rule {
            source_document: Some("docs/awards/schads.md".to_string()),
            source_section: Some("Clause 4.2".to_string()),
            ..rule("Pay overtime after the threshold")
        };

        let summary = serde_json::to_value(RuleSummary::of(&cited)).unwrap();

        assert_eq!(summary["id"], "rule_overtime");
        assert_eq!(summary["status"], "active");
        assert_eq!(summary["severity"], "high");
        assert_eq!(summary["statement"], "Pay overtime after the threshold");
        assert_eq!(summary["source"]["document"], "docs/awards/schads.md");
        assert_eq!(summary["source"]["section"], "Clause 4.2");
    }
}
