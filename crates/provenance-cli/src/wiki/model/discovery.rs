use super::{GapNotice, OrphanReport, PageLink};
use provenance_core::QuestionStatus;
use serde::Serialize;

pub const HOMEPAGE_DOMAIN_ROW_CAP: usize = 20;

/// One requirement or rule in the offline full-text index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchEntry {
    pub link: PageLink,
    pub statement: String,
}

/// Search data rendered directly into the static page DOM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchIndexPage {
    pub scope: String,
    pub title: String,
    pub coverage: String,
    pub example: Option<String>,
    pub entries: Vec<SearchEntry>,
}

/// Every recorded decision, listed by title with its position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionIndexPage {
    pub scope: String,
    pub title: String,
    pub entries: Vec<SearchEntry>,
}

/// One unresolved question with the requirement that owns it when present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenQuestionNotice {
    pub question: String,
    pub status: QuestionStatus,
    pub requirement: Option<PageLink>,
}

/// The scope's gaps, disconnected records, and unanswered questions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnfinishedPage {
    pub scope: String,
    pub title: String,
    pub gaps: Vec<GapNotice>,
    pub orphans: OrphanReport,
    pub open_questions: Vec<OpenQuestionNotice>,
}

impl UnfinishedPage {
    pub const fn item_count(&self) -> usize {
        self.gaps.len() + self.orphans.sources.len() + self.open_questions.len()
    }
}

/// The metadata available for one reader-facing Domain group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum DomainState {
    Defined {
        id: String,
        name: String,
        description: Option<String>,
    },
    Missing {
        id: String,
    },
    Unassigned,
}

/// A Domain and records placed there through requirement provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainGroup {
    pub state: DomainState,
    pub requirements: Vec<PageLink>,
    pub rules: Vec<PageLink>,
}

/// Reader taxonomy for one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainIndexPage {
    pub scope: String,
    pub title: String,
    pub authored_group_count: usize,
    pub groups: Vec<DomainGroup>,
    pub all_requirements: Vec<SearchEntry>,
    pub all_rules: Vec<SearchEntry>,
}

/// One authored domain summarized on the bounded homepage browse list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HomepageDomain {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub requirements: usize,
    pub rules: usize,
}
