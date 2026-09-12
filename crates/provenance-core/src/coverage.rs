use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};

/// Durable identity for one source line, independent of its line number.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct EvidenceAnchor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub content_hash: String,
}

impl EvidenceAnchor {
    pub fn new(symbol: Option<String>, line: &str) -> Self {
        let digest = Sha256::digest(line.trim().as_bytes());
        Self {
            symbol,
            content_hash: format!("sha256:{digest:x}"),
        }
    }
}

/// What a later scan learned when resolving a durable evidence anchor.
///
/// `New` is what a scan says when it has nothing to compare against: no
/// baseline site shares this site's anchor. Every site in a scan run without
/// `--baseline` is `New`, because such a scan knows nothing about history.
/// `Unchanged` is reserved for a site pinned to a baseline site, so it never
/// claims more than the scan checked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorState {
    #[default]
    Unchanged,
    New,
    Moved,
    Gone,
}

/// How one graph evidence path relates to a selected Git diff.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDiffState {
    #[default]
    Untouched,
    Touched,
    Moved,
    Gone,
}

/// The graph relationship that makes a path evidence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSiteKind {
    RuleBinding,
    Verification,
    Annotation,
    SourceReference,
}

/// One graph-cited evidence site resolved against both ends of a diff.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct EvidenceDiffSite {
    pub kind: EvidenceSiteKind,
    pub subject_id: String,
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub file_path: Utf8PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    pub state: EvidenceDiffState,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<String>"))]
    pub original_file_path: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_line: Option<usize>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct EvidenceDiffSummary {
    pub total_sites: usize,
    pub untouched: usize,
    pub touched: usize,
    pub moved: usize,
    pub gone: usize,
}

/// Read-only answer to whether a Git diff intersects graph evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct EvidenceDiffReport {
    pub base: String,
    pub head: String,
    pub files_changed: usize,
    pub summary: EvidenceDiffSummary,
    pub sites: Vec<EvidenceDiffSite>,
}

/// Something the scan wants to say about a rule.
///
/// `file_path` and `line` are `None` when the warning is about an absence.
/// An unverified rule has no site to point at, and naming one anyway sends a
/// reader to a file that says nothing about the problem.
///
/// `binding_finding` marks the warnings the Rule binding lifecycle policy
/// governs: an active Rule with no current verification, a current
/// implementation or verification binding to a deprecated or archived Rule,
/// and a current typed implementation or verification binding to a retired
/// Rule. Repository configuration selects warning or error severity for these
/// findings; every other warning stays report-only unless `--strict` runs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ValidationWarning {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub message: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub binding_finding: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AnnotationResult {
    pub rule_id: String,
    pub file_path: Utf8PathBuf,
    pub line: usize,
    pub function_name: Option<String>,
    pub coverage: String,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<EvidenceAnchor>,
    #[serde(default)]
    pub anchor_state: AnchorState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_file_path: Option<Utf8PathBuf>,
}

/// A `#[rule]` or `#[verifies]` attribute site. `verification` is `None` for
/// a primary implementation binding and the method word for a verification
/// binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct BindingResult {
    pub rule_id: String,
    pub file_path: Utf8PathBuf,
    pub line: usize,
    pub item_name: Option<String>,
    pub verification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<EvidenceAnchor>,
    #[serde(default)]
    pub anchor_state: AnchorState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_file_path: Option<Utf8PathBuf>,
}

/// One source file read by the scan. Keeping its content in the report lets
/// offline consumers show the evidence without relying on a code host.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ScannedFile {
    pub file_path: Utf8PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CoverageReport {
    pub commit: Option<String>,
    pub files_scanned: usize,
    pub total_annotations: usize,
    pub warnings: Vec<ValidationWarning>,
    pub annotations: Vec<AnnotationResult>,
    pub bindings: Vec<BindingResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_bindings: Vec<crate::VerificationBinding>,
}

/// A report plus the exact source text read by this scan.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CoverageScan {
    #[serde(flatten)]
    pub report: CoverageReport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scanned_files: Vec<ScannedFile>,
}

impl std::ops::Deref for CoverageScan {
    type Target = CoverageReport;

    fn deref(&self) -> &Self::Target {
        &self.report
    }
}

impl CoverageReport {
    pub const fn new(
        commit: Option<String>,
        files_scanned: usize,
        annotations: Vec<AnnotationResult>,
        bindings: Vec<BindingResult>,
        warnings: Vec<ValidationWarning>,
    ) -> Self {
        Self {
            commit,
            files_scanned,
            total_annotations: annotations.len(),
            warnings,
            annotations,
            bindings,
            verification_bindings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_report_counts_annotations() {
        let report = CoverageReport::new(
            Some("abc123".into()),
            2,
            vec![AnnotationResult {
                rule_id: "rule_overtime".into(),
                file_path: Utf8PathBuf::from("src/payroll.rs"),
                line: 4,
                function_name: Some("pays_overtime".into()),
                coverage: "full".into(),
                confidence: 1.0,
                verification: None,
                anchor: None,
                anchor_state: AnchorState::Unchanged,
                original_line: None,
                original_file_path: None,
            }],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(report.total_annotations, 1);
    }

    #[test]
    fn old_annotation_results_default_to_implementation_role() {
        let annotation: AnnotationResult = serde_json::from_str(
            r#"{
                "rule_id": "rule_overtime",
                "file_path": "src/payroll.rs",
                "line": 4,
                "function_name": "pays_overtime",
                "coverage": "full",
                "confidence": 1.0
            }"#,
        )
        .unwrap();

        assert_eq!(annotation.verification, None);
    }
}
