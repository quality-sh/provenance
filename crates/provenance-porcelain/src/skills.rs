//! Skills: list, show, and install the agent skills that the program bundles.

use crate::export::Document;
use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The `skills list` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsListInput {
    pub format: String,
}

/// The `skills show` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsShowInput {
    pub name: String,
}

/// The `skills install` input. Each bool is one switch.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsInstallInput {
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub force: bool,
    pub format: String,
}

/// One bundled skill: its name and the description from its front matter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
}

/// What an install run did to one file.
///
/// These five outcomes are the whole vocabulary of an install report; the status
/// of the run is the strongest of them. The serialised names are the strings the JSON report has always carried,
/// so the wire format does not depend on the Rust spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// Already byte-for-byte what provenance would write.
    Unchanged,
    /// Newly written where nothing was.
    Installed,
    /// Rewritten over something else.
    Updated,
    /// A new symlink into the canonical directory.
    Linked,
    /// A legacy file deleted by cleanup.
    Removed,
}

/// What one install run did: the target directories, the link mode, and each file.
#[derive(Clone, Debug, Serialize)]
pub struct InstallReport {
    pub global: bool,
    pub status: FileStatus,
    pub canonical_dir: String,
    pub claude_dir: String,
    pub link_mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    pub files: Vec<FileInstallReport>,
}

impl InstallReport {
    /// The report of each file.
    pub fn files(&self) -> &[FileInstallReport] {
        &self.files
    }
}

/// What an install run did to one file.
#[derive(Clone, Debug, Serialize)]
pub struct FileInstallReport {
    pub path: String,
    pub status: FileStatus,
}

/// The port that lists, shows, and installs the bundled skills.
pub trait SkillsPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn skills_list(
        &self,
        input: SkillsListInput,
    ) -> WorkFuture<'_, Vec<SkillSummary>, Self::Failure>;

    fn skills_show(&self, input: SkillsShowInput) -> WorkFuture<'_, Document, Self::Failure>;

    fn skills_install(
        &self,
        input: SkillsInstallInput,
    ) -> WorkFuture<'_, InstallReport, Self::Failure>;
}

impl<P: SkillsPort> crate::Porcelain<P> {
    /// Lists the bundled skills in name order.
    pub async fn skills_list(
        &self,
        input: SkillsListInput,
    ) -> Result<Vec<SkillSummary>, P::Failure> {
        self.port.skills_list(input).await
    }

    /// Gives the `SKILL.md` text of one bundled skill.
    pub async fn skills_show(&self, input: SkillsShowInput) -> Result<Document, P::Failure> {
        self.port.skills_show(input).await
    }

    /// Installs the bundled skills in the current directory, or in the home directory
    /// when the input is global.
    pub async fn skills_install(
        &self,
        input: SkillsInstallInput,
    ) -> Result<InstallReport, P::Failure> {
        self.port.skills_install(input).await
    }
}
