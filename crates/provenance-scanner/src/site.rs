use camino::Utf8Path;

use crate::{AnnotationLocation, AttributeBinding, FileScan, Verification};
/// The semantic role of a native attribute or portable comment marker.
pub use provenance_core::coverage::SiteRole as SourceSiteRole;

/// One source relationship, independent of the syntax that declared it.
#[derive(Debug, Clone, Copy)]
pub enum SourceSite<'a> {
    Annotation(&'a AnnotationLocation),
    Attribute(&'a AttributeBinding),
}

impl<'a> SourceSite<'a> {
    pub const fn role(self) -> SourceSiteRole {
        if self.verification().is_some() {
            SourceSiteRole::Verification
        } else {
            SourceSiteRole::Implementation
        }
    }

    pub fn rule_id(self) -> &'a str {
        match self {
            Self::Annotation(site) => &site.annotation.rule,
            Self::Attribute(site) => &site.rule_id,
        }
    }

    pub fn file_path(self) -> &'a Utf8Path {
        match self {
            Self::Annotation(site) => &site.file_path,
            Self::Attribute(site) => &site.file_path,
        }
    }

    pub const fn line(self) -> usize {
        match self {
            Self::Annotation(site) => site.line,
            Self::Attribute(site) => site.line,
        }
    }

    pub const fn verification(self) -> Option<Verification> {
        match self {
            Self::Annotation(site) => site.annotation.verification,
            Self::Attribute(site) => site.verification,
        }
    }

    pub fn item_name(self) -> Option<&'a str> {
        match self {
            Self::Annotation(site) => site.function_name.as_deref(),
            Self::Attribute(site) => site.item_name.as_deref(),
        }
    }
}

/// Every source relationship in scan order, with native and portable syntax
/// projected onto the same role classification.
pub fn source_sites(scans: &[FileScan]) -> impl Iterator<Item = SourceSite<'_>> {
    scans.iter().flat_map(|scan| {
        let mut sites = scan
            .annotations
            .iter()
            .map(SourceSite::Annotation)
            .chain(scan.bindings.iter().map(SourceSite::Attribute))
            .collect::<Vec<_>>();
        sites.sort_by_key(|site| site.line());
        sites.into_iter()
    })
}
