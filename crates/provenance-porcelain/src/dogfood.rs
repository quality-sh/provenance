//! Dogfood: record, report, and triage pain-point notes about provenance itself, and
//! try the first-run experience in a sandbox. Only development builds have these
//! commands.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The kind of pain that one note records.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Friction,
    Confusion,
    Missing,
    Bug,
    Idea,
}

/// The impact that the pain had on the task.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Blocked,
    Workaround,
    Annoyance,
}

/// The triage state that a triage list shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TriageStatus {
    Unhandled,
    Handled,
    All,
}

/// The `dogfood note` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodNoteInput {
    pub surface: String,
    pub category: Category,
    pub severity: Severity,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
    pub summary: String,
}

/// The `dogfood list` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodListInput {
    pub format: String,
}

/// The `dogfood report` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodReportInput {
    pub enrich: Option<String>,
    pub format: String,
}

/// The `dogfood triage handle` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriageHandleInput {
    pub id: String,
    pub reason: Option<String>,
}

/// The `dogfood triage reopen` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriageReopenInput {
    pub id: String,
}

/// The `dogfood triage list` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriageListInput {
    pub status: TriageStatus,
    pub format: String,
}

/// The `dogfood simulate` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodSimulateInput {
    #[serde(default)]
    pub keep: bool,
    pub dir: Option<String>,
}

/// The enrichment document of a report. When the `enrich` field is `-`, the caller
/// reads the document from its own input and gives the text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Enrichment {
    None,
    File(String),
    Text(String),
}

/// One captured note, as the spool holds it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub ts_ms: i64,
    pub session_id: Option<String>,
    pub host: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub provenance_version: String,
    pub surface: String,
    pub category: Category,
    pub severity: Severity,
    pub summary: String,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
}

/// One note of a report, with the session data that the enrichment gives for it.
#[derive(Clone, Debug, Serialize)]
pub struct ReportNote {
    #[serde(flatten)]
    pub note: Note,
    pub session: Option<serde_json::Value>,
}

/// The number of notes with one surface, category, and severity.
#[derive(Clone, Debug, Serialize)]
pub struct ReportBucket {
    pub surface: String,
    pub category: Category,
    pub severity: Severity,
    pub count: usize,
}

/// The aggregate of the spool.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub total: usize,
    pub counts: Vec<ReportBucket>,
    pub notes: Vec<ReportNote>,
}

/// One note as the triage view shows it: the identifier to act on, the current
/// state, and enough of the note to recognize it.
#[derive(Clone, Debug, Serialize)]
pub struct TriageView {
    pub id: String,
    pub status: &'static str,
    pub reason: Option<String>,
    pub ts_ms: i64,
    pub session_id: Option<String>,
    pub surface: String,
    pub category: Category,
    pub severity: Severity,
    pub summary: String,
}

/// The spool file that a note went into.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NoteRecorded {
    pub spool: String,
}

/// The new triage state of one note.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TriageMarked {
    pub id: String,
    pub handled: bool,
}

/// The readable account of one sandbox run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Simulation {
    pub text: String,
}

/// The port of the dogfood commands.
pub trait DogfoodPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn note(&self, input: DogfoodNoteInput) -> WorkFuture<'_, NoteRecorded, Self::Failure>;

    fn list(&self, input: DogfoodListInput) -> WorkFuture<'_, Vec<Note>, Self::Failure>;

    fn report(
        &self,
        input: DogfoodReportInput,
        enrichment: Enrichment,
    ) -> WorkFuture<'_, Report, Self::Failure>;

    fn triage_mark(
        &self,
        id: String,
        handled: bool,
        reason: Option<String>,
    ) -> WorkFuture<'_, TriageMarked, Self::Failure>;

    fn triage_list(&self, input: TriageListInput)
        -> WorkFuture<'_, Vec<TriageView>, Self::Failure>;

    fn simulate(&self, input: DogfoodSimulateInput) -> WorkFuture<'_, Simulation, Self::Failure>;
}

impl<P: DogfoodPort> crate::Porcelain<P> {
    /// Appends one note to the local spool.
    pub async fn dogfood_note(&self, input: DogfoodNoteInput) -> Result<NoteRecorded, P::Failure> {
        self.port.note(input).await
    }

    /// Gives each note of the spool.
    pub async fn dogfood_list(&self, input: DogfoodListInput) -> Result<Vec<Note>, P::Failure> {
        self.port.list(input).await
    }

    /// Counts the notes of the spool and joins the session data of the enrichment.
    pub async fn dogfood_report(
        &self,
        input: DogfoodReportInput,
        enrichment: Enrichment,
    ) -> Result<Report, P::Failure> {
        self.port.report(input, enrichment).await
    }

    /// Marks one note handled.
    pub async fn dogfood_triage_handle(
        &self,
        input: TriageHandleInput,
    ) -> Result<TriageMarked, P::Failure> {
        self.port.triage_mark(input.id, true, input.reason).await
    }

    /// Returns one handled note to the unhandled state.
    pub async fn dogfood_triage_reopen(
        &self,
        input: TriageReopenInput,
    ) -> Result<TriageMarked, P::Failure> {
        self.port.triage_mark(input.id, false, None).await
    }

    /// Gives each note with its identifier and triage state.
    pub async fn dogfood_triage_list(
        &self,
        input: TriageListInput,
    ) -> Result<Vec<TriageView>, P::Failure> {
        self.port.triage_list(input).await
    }

    /// Runs the first-run experience in a sandbox.
    pub async fn dogfood_simulate(
        &self,
        input: DogfoodSimulateInput,
    ) -> Result<Simulation, P::Failure> {
        self.port.simulate(input).await
    }
}
