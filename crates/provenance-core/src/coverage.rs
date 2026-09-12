use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::ops::{Deref, DerefMut};

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
/// governs: an active Rule with no current verification, and a current
/// implementation or verification binding to a deprecated or archived Rule.
/// Repository configuration selects warning or error severity for these
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

/// Fields shared by each scanned annotation and binding site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SiteCore {
    pub rule_id: String,
    pub file_path: Utf8PathBuf,
    pub line: usize,
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

/// A result that exposes shared evidence-site fields through its `SiteCore`.
pub trait AnchoredSite: Clone + Deref<Target = SiteCore> + DerefMut {
    fn rule_id(&self) -> &str {
        &self.deref().rule_id
    }

    fn file_path(&self) -> &camino::Utf8Path {
        &self.deref().file_path
    }

    fn line(&self) -> usize {
        self.deref().line
    }

    fn anchor(&self) -> Option<&EvidenceAnchor> {
        self.deref().anchor.as_ref()
    }

    fn mark(
        &mut self,
        state: AnchorState,
        original_line: Option<usize>,
        original_file_path: Option<Utf8PathBuf>,
    ) {
        let site = self.deref_mut();
        site.anchor_state = state;
        site.original_line = original_line;
        site.original_file_path = original_file_path;
    }
}

impl<T> AnchoredSite for T where T: Clone + Deref<Target = SiteCore> + DerefMut {}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct AnnotationResult {
    #[serde(flatten)]
    pub site: SiteCore,
    pub function_name: Option<String>,
    pub coverage: String,
    pub confidence: f64,
}

/// A `#[rule]` or `#[verifies]` attribute site. `verification` is `None` for
/// a primary implementation binding and the method word for a verification
/// binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct BindingResult {
    #[serde(flatten)]
    pub site: SiteCore,
    pub item_name: Option<String>,
}

impl Deref for AnnotationResult {
    type Target = SiteCore;

    fn deref(&self) -> &Self::Target {
        &self.site
    }
}

impl DerefMut for AnnotationResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.site
    }
}

impl Deref for BindingResult {
    type Target = SiteCore;

    fn deref(&self) -> &Self::Target {
        &self.site
    }
}

impl DerefMut for BindingResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.site
    }
}

#[derive(serde::Serialize)]
struct SerializedSite<'a, D> {
    rule_id: &'a str,
    file_path: &'a Utf8PathBuf,
    line: usize,
    #[serde(flatten)]
    details: D,
    #[serde(skip_serializing_if = "Option::is_none")]
    anchor: Option<&'a EvidenceAnchor>,
    anchor_state: AnchorState,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_file_path: Option<&'a Utf8PathBuf>,
}

impl<'a, D> SerializedSite<'a, D> {
    fn new(site: &'a SiteCore, details: D) -> Self {
        Self {
            rule_id: &site.rule_id,
            file_path: &site.file_path,
            line: site.line,
            details,
            anchor: site.anchor.as_ref(),
            anchor_state: site.anchor_state,
            original_line: site.original_line,
            original_file_path: site.original_file_path.as_ref(),
        }
    }
}

impl serde::Serialize for AnnotationResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct Details<'a> {
            function_name: Option<&'a str>,
            coverage: &'a str,
            confidence: f64,
            #[serde(skip_serializing_if = "Option::is_none")]
            verification: Option<&'a str>,
        }

        serde::Serialize::serialize(
            &SerializedSite::new(
                &self.site,
                Details {
                    function_name: self.function_name.as_deref(),
                    coverage: &self.coverage,
                    confidence: self.confidence,
                    verification: self.verification.as_deref(),
                },
            ),
            serializer,
        )
    }
}

impl serde::Serialize for BindingResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct Details<'a> {
            item_name: Option<&'a str>,
            verification: Option<&'a str>,
        }

        serde::Serialize::serialize(
            &SerializedSite::new(
                &self.site,
                Details {
                    item_name: self.item_name.as_deref(),
                    verification: self.verification.as_deref(),
                },
            ),
            serializer,
        )
    }
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
                site: SiteCore {
                    rule_id: "rule_overtime".into(),
                    file_path: Utf8PathBuf::from("src/payroll.rs"),
                    line: 4,
                    verification: None,
                    anchor: None,
                    anchor_state: AnchorState::Unchanged,
                    original_line: None,
                    original_file_path: None,
                },
                function_name: Some("pays_overtime".into()),
                coverage: "full".into(),
                confidence: 1.0,
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
        assert!(!serde_json::to_string(&annotation)
            .unwrap()
            .contains("\"verification\""));
    }

    #[test]
    fn legacy_coverage_baseline_round_trips_byte_for_byte() {
        let json = r#"{
  "commit": "abc123",
  "files_scanned": 1,
  "total_annotations": 1,
  "warnings": [
    {
      "rule_id": "rule_warning",
      "file_path": "src/lib.rs",
      "line": 2,
      "message": "unknown rule id `rule_warning`"
    }
  ],
  "annotations": [
    {
      "rule_id": "rule_annotation",
      "file_path": "src/lib.rs",
      "line": 4,
      "function_name": "pays_overtime",
      "coverage": "full",
      "confidence": 1.0,
      "verification": "examples",
      "anchor": {
        "symbol": "pays_overtime",
        "content_hash": "sha256:annotation"
      },
      "anchor_state": "moved",
      "original_line": 3,
      "original_file_path": "src/old.rs"
    }
  ],
  "bindings": [
    {
      "rule_id": "rule_binding",
      "file_path": "src/lib.rs",
      "line": 8,
      "item_name": "checks_overtime",
      "verification": null,
      "anchor": {
        "symbol": "checks_overtime",
        "content_hash": "sha256:binding"
      },
      "anchor_state": "unchanged",
      "original_line": 7,
      "original_file_path": "src/old.rs"
    }
  ],
  "scanned_files": [
    {
      "file_path": "src/lib.rs",
      "content": "fn pays_overtime() {}\n"
    }
  ]
}"#;

        let baseline: CoverageScan = serde_json::from_str(json).unwrap();

        assert_eq!(baseline.annotations[0].site.rule_id, "rule_annotation");
        assert_eq!(baseline.bindings[0].site.rule_id, "rule_binding");
        assert_eq!(baseline.bindings[0].site.verification, None);
        assert_eq!(serde_json::to_string_pretty(&baseline).unwrap(), json);
    }
}
