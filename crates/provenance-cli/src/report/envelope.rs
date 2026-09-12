//! The versioned bounded report envelope: the input contract for the report
//! renderer.
//!
//! The envelope carries identity and limits. It excludes scanned source
//! contents. Binding presence, verification-run results and evidence
//! relevance are separate fields. Severity is payload, never identity.

use serde::{Deserialize, Serialize};

/// The only envelope schema version this renderer accepts.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Whether one path is a normalized repository-relative path. Only such a
/// path, joined with an immutable commit, may become a repository link.
/// Components are restricted to `[A-Za-z0-9._~-]` so a link label cannot
/// swallow trusted text and a link destination stays CommonMark-safe.
pub fn is_repo_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    if path.chars().any(char::is_control) {
        return false;
    }
    path.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && component
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | '-'))
    })
}

/// Whether one string can be a repository identity in `owner/name` form.
fn is_repository_identity(repository: &str) -> bool {
    let Some((owner, name)) = repository.split_once('/') else {
        return false;
    };
    let ok = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    ok(owner) && ok(name)
}

/// Whether one string is a full immutable commit hash: 40 (SHA-1) or 64
/// (SHA-256) lowercase hexadecimal characters. Abbreviations are refused so
/// every rendered link pins one unambiguous commit.
fn is_commitish(commit: &str) -> bool {
    (commit.len() == 40 || commit.len() == 64)
        && commit
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

impl ReportEnvelope {
    /// Check the envelope contract. A rejected envelope never renders.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(format!(
                "unsupported report envelope schema_version {}; this renderer \
                 supports schema version {SUPPORTED_SCHEMA_VERSION} only",
                self.schema_version
            ));
        }
        if !is_repository_identity(&self.repository) {
            return Err(format!(
                "repository {:?} is not an owner/name identity; a full URL is \
                 never accepted as a repository link source",
                self.repository
            ));
        }
        for commit in [&self.base_commit, &self.head_commit] {
            if !is_commitish(commit) {
                return Err(format!(
                    "commit {commit:?} is not a full immutable hash; use 40 or \
                     64 lowercase hexadecimal characters"
                ));
            }
        }
        if self.scan.completeness == Completeness::Incomplete
            && self
                .scan
                .incompleteness_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(
                "scan completeness is incomplete without an incompleteness_reason; \
                 state which analysis stage limited the scan"
                    .to_string(),
            );
        }
        if self.scan.baseline == BaselineCompatibility::Incompatible
            && self
                .scan
                .baseline_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(
                "baseline is incompatible without a baseline_reason; state why \
                 the two scans cannot be compared"
                    .to_string(),
            );
        }
        for run in &self.verification_runs {
            let Some(commit) = run.commit.as_deref() else {
                continue;
            };
            if !is_commitish(commit) {
                return Err(format!(
                    "verification run for {} carries commit {commit:?}, which is \
                     not a full immutable hash; use 40 or 64 lowercase \
                     hexadecimal characters",
                    run.rule_id
                ));
            }
        }
        for finding in &self.findings {
            let labels_new_or_resolved =
                matches!(finding.comparison, Comparison::New | Comparison::Resolved);
            if labels_new_or_resolved && self.scan.baseline != BaselineCompatibility::Compatible {
                return Err(format!(
                    "finding subject {} is labelled {:?}, but the baseline is \
                     not compatible; a missing or incompatible baseline cannot \
                     label a finding new or resolved",
                    finding.subject.id,
                    match finding.comparison {
                        Comparison::New => "new",
                        Comparison::Resolved => "resolved",
                        _ => "labelled",
                    }
                ));
            }
            if finding.binding_presence == BindingPresence::Absent
                && self.scan.completeness == Completeness::Incomplete
            {
                return Err(format!(
                    "finding subject {} reports an absent binding, but the scan \
                     is incomplete; an incomplete scan cannot clear an absence \
                     finding",
                    finding.subject.id
                ));
            }
            if crate::report::catalog::DiagnosticCode::parse(&finding.code).is_none() {
                return Err(format!(
                    "finding subject {} carries unknown diagnostic code {}; \
                     codes come from the reviewed catalog in code",
                    finding.subject.id, finding.code
                ));
            }
        }
        Ok(())
    }
}

/// Versioned bounded input contract for the deterministic report renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportEnvelope {
    pub schema_version: u32,
    /// Repository identity in `owner/name` form; repository links are built
    /// from this plus an immutable commit, never from an author-supplied URL.
    pub repository: String,
    pub scope: String,
    /// Comparison base commit (immutable).
    pub base_commit: String,
    /// Head commit (immutable).
    pub head_commit: String,
    pub scan: ScanFacts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<PolicyOutcome>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph_changes: Vec<GraphChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_runs: Vec<VerificationRun>,
}

/// Scan facts for the head comparison: completeness, compatibility and any
/// operational failure. These facts limit what the report may claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanFacts {
    pub completeness: Completeness,
    /// Required when completeness is `incomplete`; the analysis stage that
    /// limited the scan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incompleteness_reason: Option<String>,
    pub baseline: BaselineCompatibility,
    /// Required when baseline is `incompatible`; why the two scans cannot be
    /// compared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_reason: Option<String>,
    #[serde(default)]
    pub files_scanned: u64,
    /// An operational failure is a stage that failed. It is never rendered as
    /// zero findings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<ScanFailure>,
}

/// Whether the scan covers the declared scope. An incomplete scan cannot
/// clear an absence finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Incomplete,
}

/// Whether the baseline scan can be compared with the head scan. A missing or
/// incompatible baseline cannot label a finding new or resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineCompatibility {
    Compatible,
    Missing,
    Incompatible,
}

/// The analysis stage that failed operationally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStage {
    GraphDecode,
    GraphCheck,
    SourceScan,
    BaselineLoad,
    EvidenceCollection,
    VerificationCollection,
}

/// One operational failure with its stage and an escaped fact message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanFailure {
    pub stage: ScanStage,
    #[serde(default)]
    pub message: String,
}

/// The configured policy outcome, supplied by the analysis layer. Rendering
/// and the policy outcome are separate layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyOutcome {
    pub mode: PolicyMode,
    pub result: PolicyResult,
}

/// Repository configuration: warning reports and succeeds, error reports and
/// fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    Warning,
    Error,
}

/// The check result under the configured mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyResult {
    Success,
    Failure,
    Unavailable,
}

/// One graph record change between the base and head snapshots. The stable
/// subject is the record kind and id; statements and lifecycle are payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphChange {
    pub kind: RecordKind,
    pub change: ChangeKind,
    pub id: String,
    /// Current statement: the added or removed statement, or the statement
    /// after a change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement: Option<String>,
    /// The statement before a change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_after: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations_added: Vec<RelationChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations_removed: Vec<RelationChange>,
}

/// Graph record kinds the report can describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Requirement,
    Rule,
    Resolution,
    Source,
}

impl RecordKind {
    /// The stable kind word used in output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::Rule => "rule",
            Self::Resolution => "resolution",
            Self::Source => "source",
        }
    }
}

/// How a record changed between the two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

impl ChangeKind {
    /// The stable change word used in output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }
}

/// One added or removed relationship, with both endpoints named.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationChange {
    pub relation: String,
    pub target_kind: RecordKind,
    pub target_id: String,
}

/// One finding. The stable subject is the diagnostic code plus the subject
/// identity; severity, comparison, presence, relevance and sites are payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// A code from the reviewed diagnostic-code catalog in code.
    pub code: String,
    pub subject: Subject,
    pub severity: Severity,
    pub comparison: Comparison,
    pub binding_presence: BindingPresence,
    /// The affected obligation statement, quoted as data. The renderer never
    /// takes prose from graph text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement: Option<String>,
    /// Whether changed-intent review touches this subject. Separate fact.
    #[serde(default)]
    pub relevance: Relevance,
    /// Current evidence sites (head, or unchanged base sites).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sites: Vec<Site>,
    /// Base evidence sites that are gone at the head.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_sites: Vec<Site>,
    /// The Rule a requirement-subject finding affects, for grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_rule_id: Option<String>,
    /// The Requirement a rule-subject finding refines, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_requirement_id: Option<String>,
}

/// Stable finding subject identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub kind: SubjectKind,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Rule,
    Requirement,
}

/// Configured severity for this finding. Payload, not identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

/// Comparison label. New and resolved require a compatible baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    New,
    PreExisting,
    Resolved,
    Uncertain,
}

/// Whether a current binding names the subject now. A historical retired
/// relationship is not presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingPresence {
    Present,
    Absent,
    Unknown,
}

/// Whether changed-intent review touches this subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relevance {
    #[default]
    None,
    ReviewRequested,
}

/// One evidence site. The path is a normalized repository-relative path; the
/// commit is a role resolved to the immutable envelope commits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub commit: CommitRole,
    pub path: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<SiteRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitRole {
    Base,
    Head,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteRole {
    Implementation,
    Verification,
}

/// One verification-run fact. Separate from binding presence. A run at a
/// different commit is not a pass for the head.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRun {
    pub rule_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Passed,
    Failed,
}
