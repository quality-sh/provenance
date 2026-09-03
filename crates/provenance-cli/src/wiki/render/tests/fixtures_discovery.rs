use crate::wiki::model::{
    DecisionIndexPage, GapKind, GapNotice, OrphanRecord, OrphanReport, PageKind, SearchEntry,
    SearchIndexPage, UnfinishedPage,
};

use super::fixtures::link;

pub(super) fn search_fixture() -> SearchIndexPage {
    SearchIndexPage {
        scope: "default".to_string(),
        title: "Search project records".to_string(),
        coverage: "Search covers requirements, decisions, rules, and sources.".to_string(),
        example: Some("Invoice & participant".to_string()),
        entries: vec![SearchEntry {
            link: link(
                PageKind::Requirement,
                "req_saveinvoice_split",
                "Invoice & participant",
            ),
            statement: "Invoice & participant statement".to_string(),
        }],
    }
}

pub(super) fn decisions_fixture() -> DecisionIndexPage {
    DecisionIndexPage {
        scope: "default".to_string(),
        title: "Decisions".to_string(),
        entries: vec![SearchEntry {
            link: link(PageKind::Resolution, "res_split", "Per-portion split"),
            statement: "Adopt the split".to_string(),
        }],
    }
}

pub(super) fn unfinished_fixture() -> UnfinishedPage {
    UnfinishedPage {
        scope: "default".to_string(),
        title: "Unfinished".to_string(),
        gaps: vec![
            GapNotice {
                kind: GapKind::DanglingReference,
                subject: None,
                related: None,
                detail: "A requirement points to a source that is missing.".to_string(),
            },
            GapNotice {
                kind: GapKind::MissingSourceRefs,
                subject: None,
                related: None,
                detail: "A requirement has no source references.".to_string(),
            },
            GapNotice {
                kind: GapKind::NoResolvingDecision,
                subject: None,
                related: None,
                detail: "A requirement is marked resolved but has no resolving decision."
                    .to_string(),
            },
        ],
        orphans: OrphanReport {
            sources: vec![OrphanRecord {
                link: link(PageKind::Source, "source_unused", "Unused API spec"),
                reason: "referenced by nothing".to_string(),
            }],
        },
        open_questions: vec![],
    }
}
