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
    /// Writes the plan: the skills, AGENTS.md, .gitignore, the dictionary
    /// reference, and the manifest last. In an initialized repository, init
    /// holds the publication lock one time: it checks that the manifest did
    /// not change after the plan, validates the planned manifest against the
    /// graph, writes the project files, and publishes the manifest through
    /// the store. A read waits for init and gets the graph. A stop before the
    /// manifest leaves the old manifest.
    #[rule("rule_init_apply_rolls_back_owned_changes")]
    #[rule("rule_read_waits_for_repeated_init")]
    #[rule("rule_init_validates_planned_repository")]
    pub(in crate::porcelain) fn apply(self) -> anyhow::Result<InitEnding> {
        let layout = ProvenanceLayout::new(self.path.clone());
        // A new repository has no `.provenance`, no graph and no readers.
        // `prepare_init` validated its plan with no lock, so that a refused
        // plan creates no path.
        match std::fs::symlink_metadata(layout.provenance_dir()) {
            Ok(_) => provenance_store::repository_init::with_init_section(&layout, |section| {
                self.check_in(section, &layout)?;
                self.write(|_, bytes| section.publish_manifest(bytes.to_vec()))
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let manifest_path = layout.manifest_path();
                self.planned
                    .manifest_before
                    .recheck(manifest_path.as_std_path())?;
                let before = self.planned.manifest_before.clone();
                self.write(|rollback, bytes| {
                    rollback.replace(manifest_path.as_std_path(), &before, bytes)
                })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Refuses the plan when the manifest changed after the plan, or when the
    /// graph in the section does not allow the planned manifest.
    fn check_in(
        &self,
        section: &InitSection<'_>,
        layout: &ProvenanceLayout,
    ) -> anyhow::Result<()> {
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

    /// Writes the project files, then the manifest with `write_manifest`. On
    /// a failure, init restores each project file that it wrote.
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
