//! The skills adapter: list, show, and install the skills that the program bundles.

use super::SetupPort;
use provenance_porcelain::export::Document;
use provenance_porcelain::skills::{
    SkillSummary, SkillsInstallInput, SkillsListInput, SkillsPort, SkillsShowInput,
};
use provenance_porcelain::WorkFuture;

impl SkillsPort for SetupPort {
    type Failure = anyhow::Error;

    fn skills_list(
        &self,
        _input: SkillsListInput,
    ) -> WorkFuture<'_, Vec<SkillSummary>, anyhow::Error> {
        Box::pin(async move {
            crate::skills::list(self.skills)
        })
    }

    fn skills_show(&self, input: SkillsShowInput) -> WorkFuture<'_, Document, anyhow::Error> {
        Box::pin(async move {
            Ok(Document {
                text: crate::skills::show(self.skills, &input.name)?.to_owned(),
            })
        })
    }

    fn skills_install(
        &self,
        input: SkillsInstallInput,
    ) -> WorkFuture<'_, serde_json::Value, anyhow::Error> {
        let skills = self.skills;
        Box::pin(async move {
            let report = tokio::task::spawn_blocking(move || {
                crate::skills::install(skills, input.global, input.force, input.copy)
            })
            .await??;
            Ok(serde_json::to_value(report)?)
        })
    }
}
