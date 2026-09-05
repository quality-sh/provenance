mod binding_lexer;
mod site;

pub mod parser;
mod string_context;
pub mod validate;
pub mod walker;

pub use parser::{
    parse_annotations, Annotation, CoverageLevel, ParseResult, ParseWarning, Verification,
};
pub use site::{source_sites, SourceSite, SourceSiteRole};
pub use validate::{validate_annotations, validate_bindings, ValidationWarning};
pub use walker::{
    scan_file, scan_path, scan_path_bounded, scan_path_with_content, AnnotationLocation,
    AttributeBinding, FileScan, FileScanWithContent, Language,
};
