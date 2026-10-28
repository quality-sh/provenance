//! Shared ASD-STE100 analysis for statements changed from a comparison base.

use provenance_core::{Requirement, Rule, ScopeId};
use provenance_macros::rule;
use provenance_ste100::{
    check_descriptive, check_descriptive_with_dictionary, DictionaryImport, FindingKind,
    RuleNumber, Span, Standard, StandardIssue,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Checks a statement with an optional project dictionary.
#[must_use]
pub fn check_statement(
    statement: &str,
    dictionary: Option<&DictionaryImport>,
) -> provenance_ste100::Report {
    dictionary.map_or_else(
        || check_descriptive(statement),
        |dictionary| check_descriptive_with_dictionary(statement, dictionary),
    )
}

/// Applies the same statement rules and dictionary selection as direct writes.
#[must_use]
pub fn check_project_statement(
    layout: &crate::layout::ProvenanceLayout,
    statement: &str,
) -> provenance_ste100::Report {
    let dictionary = crate::dictionary_reference::load_project_dictionary(layout);
    check_statement(statement, dictionary.as_ref())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementRecordKind {
    Requirement,
    Rule,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StatementDiagnostic {
    pub resource_kind: StatementRecordKind,
    pub scope_id: ScopeId,
    pub id: String,
    pub field: &'static str,
    pub standard: Standard,
    pub issue: StandardIssue,
    pub analyzer_version: String,
    pub rule: RuleNumber,
    pub disposition: FindingKind,
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct StatementViolationReport<'a> {
    pub error: &'static str,
    pub diagnostics: &'a [StatementDiagnostic],
}

pub fn violation_error(diagnostics: &[StatementDiagnostic]) -> anyhow::Error {
    let report = StatementViolationReport {
        error: "asd_ste100_violations",
        diagnostics,
    };
    anyhow::anyhow!(serde_json::to_string(&report).expect("statement report is serializable"))
}

/// Checks only added records and records whose statement bytes changed.
#[must_use]
#[rule("rule_ste_changed_statement_selection")]
pub fn analyze_changed_statements(
    base_requirements: &[Requirement],
    base_rules: &[Rule],
    candidate_requirements: &[Requirement],
    candidate_rules: &[Rule],
    dictionary: Option<&DictionaryImport>,
) -> Vec<StatementDiagnostic> {
    let requirement_base = statements_by_id(base_requirements, |record| {
        (
            record.scope_id.as_str(),
            record.id.as_str(),
            record.statement.as_str(),
        )
    });
    let rule_base = statements_by_id(base_rules, |record| {
        (
            record.scope_id.as_str(),
            record.id.as_str(),
            record.statement.as_str(),
        )
    });

    let mut diagnostics = Vec::new();
    analyze_records(
        StatementRecordKind::Requirement,
        &requirement_base,
        candidate_requirements.iter().map(|record| {
            (
                &record.scope_id,
                record.id.as_str(),
                record.statement.as_str(),
            )
        }),
        dictionary,
        &mut diagnostics,
    );
    analyze_records(
        StatementRecordKind::Rule,
        &rule_base,
        candidate_rules.iter().map(|record| {
            (
                &record.scope_id,
                record.id.as_str(),
                record.statement.as_str(),
            )
        }),
        dictionary,
        &mut diagnostics,
    );
    diagnostics.sort_by(|left, right| {
        (
            left.resource_kind,
            left.scope_id.as_str(),
            left.id.as_str(),
            left.span.start,
            left.span.end,
        )
            .cmp(&(
                right.resource_kind,
                right.scope_id.as_str(),
                right.id.as_str(),
                right.span.start,
                right.span.end,
            ))
    });
    diagnostics
}

fn statements_by_id<'a, T>(
    records: &'a [T],
    fields: impl Fn(&'a T) -> (&'a str, &'a str, &'a str),
) -> BTreeMap<(&'a str, &'a str), &'a str> {
    records
        .iter()
        .map(fields)
        .map(|(scope, id, statement)| ((scope, id), statement))
        .collect()
}

fn analyze_records<'a>(
    kind: StatementRecordKind,
    base: &BTreeMap<(&str, &str), &str>,
    candidates: impl Iterator<Item = (&'a ScopeId, &'a str, &'a str)>,
    dictionary: Option<&DictionaryImport>,
    diagnostics: &mut Vec<StatementDiagnostic>,
) {
    for (scope_id, id, statement) in candidates {
        if base
            .get(&(scope_id.as_str(), id))
            .is_some_and(|base| base.as_bytes() == statement.as_bytes())
        {
            continue;
        }
        let report = check_statement(statement, dictionary);
        diagnostics.extend(
            report
                .findings
                .into_iter()
                .map(|finding| StatementDiagnostic {
                    resource_kind: kind,
                    scope_id: scope_id.clone(),
                    id: id.to_owned(),
                    field: "statement",
                    standard: report.standard,
                    issue: report.issue,
                    analyzer_version: report.analyzer_version.clone(),
                    rule: finding.rule,
                    disposition: finding.kind,
                    span: finding.span,
                    message: finding.message,
                }),
        );
    }
}
