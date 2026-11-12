//! The write step of init.
use super::{already_ending, build_summary, InitPlan};
use crate::atomic_file::FileRollbackJournal;
use crate::init_summary::InitEnding;
use crate::skills::InstallReport;
use anyhow::Context;
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository_init::InitSection;

impl InitPlan {
    /// Writes the plan and restores each file that it wrote when a write
    /// fails. In an initialized repository, a read waits for init and gets
    /// the graph from after init.
    ///
    /// The manifest is the last write, so a stop or a failure before it leaves
    /// the old manifest.
    #[rule("rule_init_apply_rolls_back_owned_changes")]
    #[rule("rule_read_waits_for_repeated_init")]
    pub(in crate::porcelain) fn apply(self) -> anyhow::Result<InitEnding> {
        let layout = ProvenanceLayout::new(self.path.clone());
        provenance_store::repository_init::with_init_section(&layout, |section| {
            if let Some(section) = section {
                self.check_in(section, &layout)?;
                return self.write(|_, bytes| section.publish_manifest(bytes.to_vec()));
            }
            // A new repository: `prepare_init` validated its plan with no
            // lock, so that a refused plan creates no path.
            let manifest_path = layout.manifest_path();
            self.planned
                .manifest_before
                .recheck(manifest_path.as_std_path())?;
            let before = self.planned.manifest_before.clone();
            self.write(|rollback, bytes| {
                rollback.replace(manifest_path.as_std_path(), &before, bytes)
            })
        })
    }

    /// Refuses a changed manifest or a planned manifest that the graph refuses.
    fn check_in(&self, section: &InitSection<'_>, layout: &ProvenanceLayout) -> anyhow::Result<()> {
        anyhow::ensure!(
            section.manifest_bytes()? == self.planned.manifest_before.bytes(),
            "{} changed after initialization was planned; retry",
            layout.manifest_path().as_std_path().display()
        );
        crate::check::validate_planned_manifest_in(section.store(), &self.manifest)
            .context("the planned Provenance state is not valid")?;
        crate::test_probes::at("init_validated");
        Ok(())
    }

    /// Writes the project files, then the manifest with `write_manifest`.
    ///
    /// On a failure, init restores each project file that it wrote.
    fn write(
        self,
        write_manifest: impl FnOnce(&mut FileRollbackJournal, &[u8]) -> anyhow::Result<()>,
    ) -> anyhow::Result<InitEnding> {
        self.skills.recheck()?;
        self.planned
            .agents_before
            .recheck(self.path.join("AGENTS.md").as_std_path())?;
        self.planned
            .gitignore_before
            .recheck(self.path.join(".gitignore").as_std_path())?;
        self.dictionary.recheck(&self.path)?;
        let mut rollback = FileRollbackJournal::within(self.path.as_std_path());
        let result = (|| -> anyhow::Result<InstallReport> {
            let skills = self.skills.apply_in(&mut rollback)?;
            let agents_path = self.path.join("AGENTS.md");
            if self.planned.agents_before.bytes() != Some(self.planned.agents_bytes.as_slice()) {
                rollback.replace(
                    agents_path.as_std_path(),
                    &self.planned.agents_before,
                    &self.planned.agents_bytes,
                )?;
            }
            let gitignore_path = self.path.join(".gitignore");
            if self.planned.gitignore_before.bytes()
                != Some(self.planned.gitignore_bytes.as_slice())
            {
                rollback.replace(
                    gitignore_path.as_std_path(),
                    &self.planned.gitignore_before,
                    &self.planned.gitignore_bytes,
                )?;
            }
            self.dictionary.apply_in(&self.path, &mut rollback)?;
            if self.planned.manifest_before.bytes() != Some(self.planned.manifest_bytes.as_slice())
            {
                write_manifest(&mut rollback, &self.planned.manifest_bytes)?;
            }
            Ok(skills)
        })();
        let skills = match result {
            Ok(skills) => skills,
            Err(error) => {
                return match rollback.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback) => Err(error.context(format!(
                        "repository initialization rollback failed: {rollback:#}"
                    ))),
                };
            }
        };
        rollback.commit()?;
        Ok(build_summary(
            &self.path,
            &self.scope_ids,
            &self.planned,
            &skills,
            &self.dictionary,
        )
        .map_or_else(
            || already_ending(&self.scope_ids, &self.path, &self.dictionary, self.command),
            |summary| InitEnding::applied(summary, self.dictionary.warning(), self.command),
        )
        .with_notice(self.reviewer_notice))
    }
}
