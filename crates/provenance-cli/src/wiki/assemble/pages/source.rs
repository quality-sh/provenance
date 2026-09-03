use crate::wiki::model::{PageId, PageLink, RecordKind, SourcePage};
use provenance_core::{NodeType, Source, StableId};

use super::super::context::Assembler;
use super::super::page_links::{requirement_link, source_link};

impl<'a> Assembler<'a> {
    pub(in crate::wiki::assemble) fn source_page(&self, source: &'a Source) -> SourcePage {
        let referenced_requirements: Vec<PageLink> = self
            .state
            .requirements
            .iter()
            .filter(|requirement| {
                requirement
                    .source_refs
                    .iter()
                    .any(|reference| reference.source_id == source.id)
            })
            .map(requirement_link)
            .collect();
        let superseded_by = self.superseding_source(&source.id).map(source_link);
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

    /// The source whose `supersedes` names this one: the first in id order
    /// when several do.
    fn superseding_source(&self, source_id: &StableId) -> Option<&'a Source> {
        self.state
            .sources
            .iter()
            .filter(|candidate| candidate.supersedes.contains(source_id))
            .min_by_key(|candidate| candidate.id.as_str())
    }
}
