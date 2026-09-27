mod binding_lexer;
mod coverage;
mod coverage_anchors;
mod rule_evidence;
mod site;

pub mod parser;
mod string_context;
pub mod validate;
pub mod walker;

pub use coverage::{scan_to_coverage, CoverageBaseline, ScannedCoverage};
pub use parser::{
    parse_annotations, Annotation, CoverageLevel, ParseResult, ParseWarning, Verification,
};
pub use rule_evidence::{
    binding_findings_fail, derive_rule_evidence_facts, scan_covers_repository,
    BindingFindingSeverity, InactiveBindingOrigin, InactiveBindingRole, InactiveCurrentBinding,
    RuleEvidenceCompleteness, RuleEvidenceFacts,
};
pub use site::{source_sites, SourceSite, SourceSiteRole};
pub use validate::{validate_annotations, validate_bindings, ValidationWarning};
pub use walker::{
    scan_file, scan_path, scan_path_bounded, scan_path_with_content, AnnotationLocation,
    AttributeBinding, FileScan, FileScanWithContent, Language,
};
