use crate::wiki::model::{PageId, PageLink, RecordKind, SourcePage};
use provenance_core::{NodeType, Source};

use super::super::context::Assembler;
use super::super::page_links::{requirement_link, source_link};

impl Assembler<'_> {
    pub(in crate::wiki::assemble) fn source_page(&self, source: &Source) -> SourcePage {
        let referenced_requirements: Vec<PageLink> = self
            .query
            .requirements_citing_source(&source.id)
            .into_iter()
            .map(requirement_link)
            .collect();
        let superseded_by = self.query.source_superseded_by(&source.id).map(source_link);
        SourcePage {
            id: PageId::new(RecordKind::Source, source.id.as_str()),
            title: source.name.clone(),
            source_type: source.source_type.clone(),
            url: source.url.clone(),
            reference: self.source_reference_link(source),
            commit_pin: source.commit_pin.clone(),
            effective_date: source.effective_date,
            review_date: source.review_date,
            superseded_by,
            referenced_requirements,
            gaps: self.gaps_for(NodeType::Source, &source.id),
            threads: self.threads_for(NodeType::Source, &source.id),
        }
    }
}
