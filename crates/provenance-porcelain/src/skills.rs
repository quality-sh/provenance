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

/// The port that lists, shows, and installs the bundled skills.
pub trait SkillsPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn skills_list(
        &self,
        input: SkillsListInput,
    ) -> WorkFuture<'_, Vec<SkillSummary>, Self::Failure>;

    fn skills_show(&self, input: SkillsShowInput) -> WorkFuture<'_, Document, Self::Failure>;

    /// The install report: what the install did to each file.
    fn skills_install(
        &self,
        input: SkillsInstallInput,
    ) -> WorkFuture<'_, serde_json::Value, Self::Failure>;
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
    ) -> Result<serde_json::Value, P::Failure> {
        self.port.skills_install(input).await
    }
}
