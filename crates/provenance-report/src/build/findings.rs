//! Mapping gathered scan, graph and evidence-diff facts into report
//! findings with catalog codes.
//!
//! The producer states only what the layers computed: absence comes from
//! the scan and the graph, statement changes come from the committed
//! snapshots, and site facts come from the evidence diff. A finding the
//! layers cannot support is left out; nothing here invents a verification
//! run, a pass, or a commit.

use crate::catalog::DiagnosticCode;
use crate::envelope::{
    is_repo_relative_path, BaselineCompatibility, BindingPresence, CommitRole, Comparison, Finding,
    Relevance, Severity, Site, SiteRole, Subject, SubjectKind,
};
use camino::Utf8Path;
use provenance_core::coverage::{EvidenceDiffReport, EvidenceDiffState, EvidenceSiteKind};
use provenance_core::{Requirement, Rule};
use provenance_scanner::{InactiveBindingOrigin, InactiveBindingRole, RuleEvidenceFacts};
use std::collections::{BTreeMap, BTreeSet};

/// How the base commit compares, for honest comparison labels.
pub(super) struct BaselineView {
    existed_at_base: Option<BTreeSet<String>>,
}

impl BaselineView {
    const fn compatible(ids: BTreeSet<String>) -> Self {
        Self {
            existed_at_base: Some(ids),
        }
    }

    const fn uncertain() -> Self {
        Self {
            existed_at_base: None,
        }
    }

    /// The comparison label for one subject. A compatible baseline labels a
    /// subject absent from the base `new` and one present at the base
    /// `pre_existing`; any other baseline stays `uncertain`.
    fn comparison(&self, subject_id: &str) -> Comparison {
        match &self.existed_at_base {
            Some(ids) if !ids.contains(subject_id) => Comparison::New,
            Some(_) => Comparison::PreExisting,
            None => Comparison::Uncertain,
        }
    }

    fn rule_id_set(rules: &[Rule]) -> BTreeSet<String> {
        rules
            .iter()
            .map(|rule| rule.id.as_str().to_string())
            .collect()
    }

    /// The baseline view for the rules that existed at the base commit.
    /// With a compatible baseline, a head rule absent from this set is new
    /// in the range; any other baseline leaves every label uncertain.
    pub(super) fn for_rules(compatibility: BaselineCompatibility, base_rules: &[Rule]) -> Self {
        match compatibility {
            BaselineCompatibility::Compatible => Self::compatible(Self::rule_id_set(base_rules)),
            BaselineCompatibility::Missing | BaselineCompatibility::Incompatible => {
                Self::uncertain()
            }
        }
    }
}

fn finding(
    code: DiagnosticCode,
    subject_kind: SubjectKind,
    subject_id: &str,
    severity: Severity,
    comparison: Comparison,
    presence: BindingPresence,
) -> Finding {
    Finding {
        code: code.as_str().to_string(),
        subject: Subject {
            kind: subject_kind,
            id: subject_id.to_string(),
        },
        severity,
        comparison,
        binding_presence: presence,
        statement: None,
        relevance: Relevance::default(),
        sites: Vec::new(),
        removed_sites: Vec::new(),
        affected_rule_id: None,
        affected_requirement_id: None,
    }
}

/// Map shared Rule evidence facts to report findings.
pub(super) fn rule_evidence_findings(
    facts: &RuleEvidenceFacts,
    baseline: &BaselineView,
    severity: Severity,
    repo: &Utf8Path,
) -> Vec<Finding> {
    let mut findings = facts
        .unimplemented
        .iter()
        .map(|rule_id| {
            finding(
                DiagnosticCode::ActiveRuleMissingImplementation,
                SubjectKind::Rule,
                rule_id,
                Severity::Warning,
                baseline.comparison(rule_id),
                BindingPresence::Absent,
            )
        })
        .chain(facts.unverified.iter().map(|rule_id| {
            finding(
                DiagnosticCode::ActiveRuleMissingVerification,
                SubjectKind::Rule,
                rule_id,
                severity,
                baseline.comparison(rule_id),
                BindingPresence::Absent,
            )
        }))
        .collect::<Vec<_>>();
    findings.extend(inactive_current_findings(facts, baseline, severity, repo));
    findings
}

fn inactive_current_findings(
    facts: &RuleEvidenceFacts,
    baseline: &BaselineView,
    severity: Severity,
    repo: &Utf8Path,
) -> Vec<Finding> {
    let mut events: BTreeMap<String, Vec<Site>> = BTreeMap::new();
    for binding in &facts.inactive_current {
        let sites = events.entry(binding.rule_id.clone()).or_default();
        if binding.origin == InactiveBindingOrigin::Scanned {
            if let Some(current) = site(
                repo,
                &binding.file_path,
                binding.line.expect("a scanned binding has a line"),
                match binding.role {
                    InactiveBindingRole::Implementation => SiteRole::Implementation,
                    InactiveBindingRole::Verification => SiteRole::Verification,
                },
                binding.verification_method.map(|method| method.to_string()),
            ) {
                sites.push(current);
            }
        }
    }
    events
        .into_iter()
        .map(|(subject_id, sites)| {
            let mut current = finding(
                DiagnosticCode::InactiveRuleCurrentBinding,
                SubjectKind::Rule,
                &subject_id,
                severity,
                baseline.comparison(&subject_id),
                BindingPresence::Present,
            );
            current.sites = sites;
            current
        })
        .collect()
}

/// Site findings from the evidence diff: verification sites that are gone or
/// moved between the base and head revisions. These facts are comparative,
/// so the caller supplies them only for a compatible baseline.
pub(super) fn evidence_site_findings(report: &EvidenceDiffReport) -> Vec<Finding> {
    report
        .sites
        .iter()
        .filter(|site| site.kind == EvidenceSiteKind::Verification)
        .filter(|site| {
            matches!(
                site.state,
                EvidenceDiffState::Gone | EvidenceDiffState::Moved
            )
        })
        .map(|site| {
            let code = match site.state {
                EvidenceDiffState::Gone => DiagnosticCode::VerificationSiteRemoved,
                _ => DiagnosticCode::VerificationSiteMoved,
            };
            let mut finding = finding(
                code,
                SubjectKind::Rule,
                &site.subject_id,
                Severity::Warning,
                Comparison::New,
                BindingPresence::Present,
            );
            if site.state == EvidenceDiffState::Moved {
                if let Some(current) = site_at(CommitRole::Head, &site.file_path, site.line) {
                    finding.sites.push(current);
                }
            }
            let origin = site
                .original_file_path
                .clone()
                .unwrap_or_else(|| site.file_path.clone());
            let origin_line = site.original_line.or(site.line);
            if let Some(removed) = site_at(CommitRole::Base, &origin, origin_line) {
                finding.removed_sites.push(removed);
            }
            finding
        })
        .collect()
}

fn site_at(commit: CommitRole, path: &Utf8Path, line: Option<usize>) -> Option<Site> {
    let line = u32::try_from(line?).ok()?;
    Some(Site {
        commit,
        path: path.to_string(),
        line,
        role: Some(SiteRole::Verification),
        method: None,
    })
}

/// Findings for requirement and rule statements whose bytes changed between
/// the committed snapshots. A change observation needs a comparable base,
/// so the caller supplies these only for a compatible baseline.
pub(super) fn statement_change_findings(
    base_requirements: &[Requirement],
    head_requirements: &[Requirement],
    base_rules: &[Rule],
    head_rules: &[Rule],
) -> Vec<Finding> {
    let base_statements: BTreeMap<&str, &str> = base_requirements
        .iter()
        .map(|record| (record.id.as_str(), record.statement.as_str()))
        .chain(
            base_rules
                .iter()
                .map(|record| (record.id.as_str(), record.statement.as_str())),
        )
        .collect();
    let mut findings = Vec::new();
    for record in head_requirements {
        if changed_statement(&base_statements, record.id.as_str(), &record.statement) {
            findings.push(statement_finding(
                SubjectKind::Requirement,
                record.id.as_str(),
                &record.statement,
                None,
            ));
        }
    }
    for record in head_rules {
        if changed_statement(&base_statements, record.id.as_str(), &record.statement) {
            let affected = record
                .requirement_ids
                .first()
                .map(provenance_core::StableId::as_str);
            findings.push(statement_finding(
                SubjectKind::Rule,
                record.id.as_str(),
                &record.statement,
                affected,
            ));
        }
    }
    findings
}

fn changed_statement(base_statements: &BTreeMap<&str, &str>, id: &str, statement: &str) -> bool {
    base_statements
        .get(id)
        .is_some_and(|before| *before != statement)
}

fn statement_finding(
    kind: SubjectKind,
    id: &str,
    statement: &str,
    affected_requirement_id: Option<&str>,
) -> Finding {
    let mut finding = finding(
        DiagnosticCode::RequirementStatementChanged,
        kind,
        id,
        Severity::Warning,
        Comparison::New,
        BindingPresence::Unknown,
    );
    finding.statement = Some(statement.to_string());
    finding.affected_requirement_id = affected_requirement_id.map(str::to_string);
    finding
}

/// Build one evidence site from a scanned marker. The path must be
/// repository-relative after the repo prefix is removed, and it must pass
/// the envelope path contract; an unsafe path yields no site rather than an
/// unsafe link.
fn site(
    repo: &Utf8Path,
    path: &Utf8Path,
    line: usize,
    role: SiteRole,
    method: Option<String>,
) -> Option<Site> {
    let relative = path.strip_prefix(repo).unwrap_or(path);
    let line = u32::try_from(line).ok()?;
    if !is_repo_relative_path(relative.as_str()) {
        return None;
    }
    Some(Site {
        commit: CommitRole::Head,
        path: relative.as_str().to_string(),
        line,
        role: Some(role),
        method,
    })
}
