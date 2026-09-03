use crate::wiki::model::{
    GapNotice, OpenQuestionNotice, OrphanRecord, OrphanReport, PageLink, UnfinishedPage,
};
use provenance_core::{NodeType, StableId};
use provenance_store::cache::{GapItem, GapKind};

use super::context::Assembler;
use super::page_links::{requirement_link, resolution_link, rule_link, source_link};
const SUBJECT_TOKEN: &str = concat!("{", "subject", "}");
const RELATED_TOKEN: &str = concat!("{", "related", "}");

impl Assembler<'_> {
    pub(super) fn unfinished_page(&self) -> UnfinishedPage {
        let gaps = self
            .gaps
            .iter()
            .filter(|gap| {
                !matches!(
                    gap.kind,
                    GapKind::UnreferencedSource | GapKind::OpenQuestion
                )
            })
            .map(|gap| self.gap_notice(gap, None))
            .collect();
        let open_questions = self
            .gaps
            .iter()
            .filter(|gap| gap.kind == GapKind::OpenQuestion)
            .filter_map(|gap| {
                self.state
                    .questions
                    .iter()
                    .find(|question| question.id.as_str() == gap.node_id)
            })
            .map(|question| OpenQuestionNotice {
                question: question.question.clone(),
                status: question.status,
                requirement: self
                    .find_requirement(&question.requirement_id)
                    .map(requirement_link),
            })
            .collect();
        UnfinishedPage {
            scope: self.state.scope.clone(),
            title: "Unfinished".to_string(),
            gaps,
            orphans: self.orphan_report(),
            open_questions,
        }
    }

    fn orphan_report(&self) -> OrphanReport {
        OrphanReport {
            sources: self
                .gaps
                .iter()
                .filter(|gap| gap.kind == GapKind::UnreferencedSource)
                .filter_map(|gap| {
                    self.state
                        .sources
                        .iter()
                        .find(|source| source.id.as_str() == gap.node_id)
                        .map(|source| OrphanRecord {
                            link: source_link(source),
                            reason: gap.reason.clone(),
                        })
                })
                .collect(),
        }
    }

    fn record_link(&self, node_type: NodeType, node_id: &str) -> Option<PageLink> {
        match node_type {
            NodeType::Requirement => self
                .state
                .requirements
                .iter()
                .find(|record| record.id.as_str() == node_id)
                .map(requirement_link),
            NodeType::Resolution => self
                .state
                .resolutions
                .iter()
                .find(|record| record.id.as_str() == node_id)
                .map(resolution_link),
            NodeType::Rule => self
                .state
                .rules
                .iter()
                .find(|record| record.id.as_str() == node_id)
                .map(rule_link),
            NodeType::Source => self
                .state
                .sources
                .iter()
                .find(|record| record.id.as_str() == node_id)
                .map(source_link),
            // Domains and boundaries have no wiki page of their own yet, so
            // a gap about one links nowhere, the same as topics and
            // questions.
            NodeType::Topic | NodeType::Question | NodeType::Domain | NodeType::Boundary => None,
        }
    }

    fn subject_link(&self, gap: &GapItem) -> Option<PageLink> {
        if matches!(gap.node_type, NodeType::Topic | NodeType::Question) {
            return gap
                .requirement_id
                .as_deref()
                .and_then(|id| self.record_link(NodeType::Requirement, id));
        }
        self.record_link(gap.node_type, &gap.node_id)
    }

    fn gap_notice(&self, gap: &GapItem, current: Option<(NodeType, &str)>) -> GapNotice {
        let mirrored = current.is_some_and(|(node_type, node_id)| {
            gap.kind == GapKind::UnresolvedContradictsPair
                && gap.related_node_type == Some(node_type)
                && gap.related_node_id.as_deref() == Some(node_id)
        });
        let owns_subject = current.is_some();
        let subject = (!owns_subject).then(|| self.subject_link(gap)).flatten();
        let related = if mirrored {
            self.record_link(gap.node_type, &gap.node_id)
        } else {
            gap.related_node_type
                .zip(gap.related_node_id.as_deref())
                .and_then(|(node_type, node_id)| self.record_link(node_type, node_id))
        };
        let subject_type = current.map_or(gap.node_type, |(node_type, _)| node_type);
        let subject_text = if owns_subject {
            format!("This {}", reader_node_word(subject_type))
        } else if subject.is_some() {
            SUBJECT_TOKEN.to_string()
        } else if gap.reason.starts_with("thread ") {
            "A discussion".to_string()
        } else {
            format!("A {}", reader_node_word(subject_type))
        };
        let detail = gap_detail(gap, &subject_text, related.is_some());

        GapNotice {
            kind: gap.kind,
            subject,
            related,
            detail,
        }
    }

    pub(super) fn gaps_for(&self, node_type: NodeType, node_id: &StableId) -> Vec<GapNotice> {
        self.gaps
            .iter()
            .filter(|gap| {
                (gap.node_type == node_type && gap.node_id == node_id.as_str())
                    || (gap.kind == GapKind::UnresolvedContradictsPair
                        && gap.related_node_type == Some(node_type)
                        && gap.related_node_id.as_deref() == Some(node_id.as_str()))
            })
            .map(|gap| self.gap_notice(gap, Some((node_type, node_id.as_str()))))
            .collect()
    }
}

fn gap_detail(gap: &GapItem, subject: &str, has_related: bool) -> String {
    match gap.kind {
        GapKind::MissingDomainId => format!("{subject} has no domain."),
        GapKind::MissingSourceRefs => format!("{subject} has no source references."),
        GapKind::NoResolvingDecision => {
            format!("{subject} is marked resolved but has no resolving decision.")
        }
        GapKind::NoProducedRules => format!("{subject} has no produced rules."),
        GapKind::UnreferencedSource => {
            format!("{subject} is not referenced by a requirement.")
        }
        GapKind::DanglingReference => {
            if gap.reason.starts_with("thread ") {
                let related = reader_node_word(gap.node_type);
                format!("{subject} belongs to a {related} that is missing.")
            } else {
                let related = gap.related_node_type.map_or("record", reader_node_word);
                format!("{subject} points to a {related} that is missing.")
            }
        }
        GapKind::UnresolvedContradictsPair => {
            let related = if has_related {
                RELATED_TOKEN
            } else {
                "another requirement"
            };
            format!("{subject} conflicts with {related}, but no decision resolves the conflict.")
        }
        GapKind::OpenQuestion => format!("{subject} has an open question."),
        GapKind::UnexploredTopic => format!("{subject} has an unexplored topic."),
    }
}

const fn reader_node_word(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "decision",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
        NodeType::Domain => "domain",
        NodeType::Boundary => "boundary",
    }
}
