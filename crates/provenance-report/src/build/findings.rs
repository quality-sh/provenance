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
use provenance_scanner::{
    source_sites, FileScan, InactiveBindingOrigin, InactiveBindingRole, RuleEvidenceFacts,
    SourceSiteRole,
};
use std::collections::{BTreeMap, BTreeSet};

/// How the base commit compares, for honest comparison labels.
pub(super) struct BaselineView {
    evidence: Option<BaselineEvidence>,
}

struct BaselineEvidence {
    rule_ids: BTreeSet<String>,
    implementation_rule_ids: BTreeSet<String>,
    verification_rule_ids: BTreeSet<String>,
}

impl BaselineView {
    const fn uncertain() -> Self {
        Self { evidence: None }
    }

    /// The comparison label for one subject. A compatible baseline labels a
    /// subject absent from the base `new` and one present at the base
    /// `pre_existing`; any other baseline stays `uncertain`.
    fn comparison(&self, subject_id: &str) -> Comparison {
        match &self.evidence {
            Some(evidence) if !evidence.rule_ids.contains(subject_id) => Comparison::New,
            Some(_) => Comparison::PreExisting,
            None => Comparison::Uncertain,
        }
    }

    fn missing_implementation_comparison(&self, rule_id: &str) -> Comparison {
        self.missing_evidence_comparison(rule_id, |evidence| &evidence.implementation_rule_ids)
    }

    fn missing_verification_comparison(&self, rule_id: &str) -> Comparison {
        self.missing_evidence_comparison(rule_id, |evidence| &evidence.verification_rule_ids)
    }

    fn missing_evidence_comparison<'a>(
        &'a self,
        rule_id: &str,
        ids: impl FnOnce(&'a BaselineEvidence) -> &'a BTreeSet<String>,
    ) -> Comparison {
        match &self.evidence {
            Some(evidence)
                if !evidence.rule_ids.contains(rule_id) || ids(evidence).contains(rule_id) =>
            {
                Comparison::New
            }
            Some(_) => Comparison::PreExisting,
            None => Comparison::Uncertain,
        }
    }

    /// The baseline view for Rules and their evidence at the base commit.
    pub(super) fn for_rules(
        compatibility: BaselineCompatibility,
        base_rules: &[Rule],
        base_scans: &[FileScan],
        base_implementations: &[provenance_core::ImplementationBinding],
        base_verifications: &[provenance_core::VerificationBinding],
    ) -> Self {
        match compatibility {
            BaselineCompatibility::Compatible => {
                let mut implementation_rule_ids = base_implementations
                    .iter()
                    .map(|binding| binding.rule_id.as_str().to_string())
                    .collect::<BTreeSet<_>>();
                let mut verification_rule_ids = base_verifications
                    .iter()
                    .map(|binding| binding.rule_id.as_str().to_string())
                    .collect::<BTreeSet<_>>();
                for site in source_sites(base_scans) {
                    match site.role() {
                        SourceSiteRole::Implementation => {
                            implementation_rule_ids.insert(site.rule_id().to_string());
                        }
                        SourceSiteRole::Verification(_) => {
                            verification_rule_ids.insert(site.rule_id().to_string());
                        }
                    }
                }
                Self {
                    evidence: Some(BaselineEvidence {
                        rule_ids: base_rules
                            .iter()
                            .map(|rule| rule.id.as_str().to_string())
                            .collect(),
                        implementation_rule_ids,
                        verification_rule_ids,
                    }),
                }
            }
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
                baseline.missing_implementation_comparison(rule_id),
                BindingPresence::Absent,
            )
        })
        .chain(facts.unverified.iter().map(|rule_id| {
            finding(
                DiagnosticCode::ActiveRuleMissingVerification,
                SubjectKind::Rule,
                rule_id,
                severity,
                baseline.missing_verification_comparison(rule_id),
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

/// Site findings from the evidence diff. Removed implementation and
/// verification sites are findings. A moved verification site is also a
/// finding. These facts need a compatible baseline.
pub(super) fn evidence_site_findings(report: &EvidenceDiffReport) -> Vec<Finding> {
    report
        .sites
        .iter()
        .filter(|site| {
            site.state == EvidenceDiffState::Gone
                && matches!(
                    site.kind,
                    EvidenceSiteKind::RuleBinding | EvidenceSiteKind::Verification
                )
                || site.kind == EvidenceSiteKind::Verification
                    && site.state == EvidenceDiffState::Moved
        })
        .map(|site| {
            let (code, role) = match (site.kind, site.state) {
                (EvidenceSiteKind::RuleBinding, EvidenceDiffState::Gone) => (
                    DiagnosticCode::ImplementationSiteRemoved,
                    SiteRole::Implementation,
                ),
                (_, EvidenceDiffState::Gone) => (
                    DiagnosticCode::VerificationSiteRemoved,
                    SiteRole::Verification,
                ),
                _ => (
                    DiagnosticCode::VerificationSiteMoved,
                    SiteRole::Verification,
                ),
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
                if let Some(current) = site_at(CommitRole::Head, &site.file_path, site.line, role) {
                    finding.sites.push(current);
                }
            }
            let origin = site
                .original_file_path
                .clone()
                .unwrap_or_else(|| site.file_path.clone());
            let origin_line = site.original_line.or(site.line);
            if let Some(removed) = site_at(CommitRole::Base, &origin, origin_line, role) {
                finding.removed_sites.push(removed);
            }
            finding
        })
        .collect()
}

fn site_at(
    commit: CommitRole,
    path: &Utf8Path,
    line: Option<usize>,
    role: SiteRole,
) -> Option<Site> {
    let line = u32::try_from(line?).ok()?;
    Some(Site {
        commit,
        path: path.to_string(),
        line,
        role: Some(role),
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
