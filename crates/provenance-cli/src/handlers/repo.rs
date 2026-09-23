use crate::atomic_file::{FileRollbackJournal, FileSnapshot};
use crate::init_summary::{scope_phrase, InitEnding, InitSummary};
use crate::skills::{FileStatus, InstallReport};
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;

pub(super) struct InitOptions {
    pub(super) scope: Option<String>,
    pub(super) path_prefix: Option<Utf8PathBuf>,
    pub(super) disposition_actor_ids: Vec<String>,
    pub(super) clear_disposition_actors: bool,
    pub(super) ste_pdf: Option<Utf8PathBuf>,
    pub(super) invocation_channel: crate::cli::InvocationChannel,
    pub(super) package_manager: Option<crate::cli::PackageManager>,
    /// Suppresses the printed summary; `--quiet` asks for silence.
    pub(super) quiet: bool,
}

pub(super) fn init(path: &Utf8Path, options: InitOptions) -> anyhow::Result<()> {
    let quiet = options.quiet;
    let plan = prepare_init(path, options)?;
    let ending = plan.apply()?;
    ending.print(quiet);
    Ok(())
}

pub(super) struct InitPlan {
    path: Utf8PathBuf,
    planned: PlannedFiles,
    skills: crate::skills::InitSkillPlan,
    dictionary: crate::ste_onboarding::Plan,
    scope_ids: Vec<String>,
}

#[rule("rule_init_plans_all_project_writes")]
#[rule("rule_init_plan_rejection_preserves_targets")]
#[rule("rule_init_validates_planned_repository")]
pub(super) fn prepare_init(path: &Utf8Path, options: InitOptions) -> anyhow::Result<InitPlan> {
    let InitOptions {
        scope,
        path_prefix,
        disposition_actor_ids,
        clear_disposition_actors,
        ste_pdf,
        invocation_channel,
        package_manager,
        ..
    } = options;
    super::check::recover_repository_before_init(path)
        .context("failed to recover an interrupted repository publication")?;
    let invocation = crate::onboarding::Invocation::from_cli(invocation_channel, package_manager)?;
    let layout = ProvenanceLayout::new(path.to_path_buf());
    let manifest_before = FileSnapshot::read(layout.manifest_path().as_std_path())?;
    let manifest_exists = manifest_before.bytes().is_some();
    anyhow::ensure!(
        manifest_exists || scope.is_some(),
        "--scope is required when initializing a new repository"
    );
    anyhow::ensure!(
        disposition_actor_ids.iter().all(|id| !id.trim().is_empty()),
        "disposition actor IDs must not be empty"
    );
    let mut manifest = if manifest_exists {
        parse_manifest(
            manifest_before
                .bytes()
                .ok_or_else(|| anyhow::anyhow!("manifest disappeared during init"))?,
        )?
    } else {
        let scope = scope.as_deref().ok_or_else(|| {
            anyhow::anyhow!("--scope is required when initializing a new repository")
        })?;
        Manifest::default_with_scope(
            ScopeId::new(scope)?,
            RepoPathPrefix::new(
                path_prefix
                    .clone()
                    .unwrap_or_else(|| Utf8PathBuf::from(".")),
            ),
        )
    };

    if manifest_exists {
        update_scope(&mut manifest, scope, path_prefix)?;
    }
    if clear_disposition_actors {
        manifest.disposition_actor_ids.clear();
    } else if !disposition_actor_ids.is_empty() {
        manifest.disposition_actor_ids = disposition_actor_ids;
    }
    let manifest_bytes = format!("{}\n", serde_json::to_string_pretty(&manifest)?).into_bytes();
    let skills = crate::skills::plan_init_at(path.as_std_path())
        .context("failed to plan the bundled Provenance skills")?;
    let planned = PlannedFiles::plan(path, &invocation, manifest_before, manifest_bytes)?;
    super::check::validate_repository_with_manifest(path, &manifest)
        .context("the planned Provenance state is not valid")?;
    let dictionary = crate::ste_onboarding::prepare(path, ste_pdf.as_deref())?;
    let scope_ids: Vec<String> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.as_str().to_owned())
        .collect();
    Ok(InitPlan {
        path: path.to_path_buf(),
        planned,
        skills,
        dictionary,
        scope_ids,
    })
}

/// The managed file states a planned init would write, kept together so the
/// summary can read them in one place.
struct PlannedFiles {
    manifest_bytes: Vec<u8>,
    manifest_before: FileSnapshot,
    agents_bytes: Vec<u8>,
    agents_before: FileSnapshot,
    agents_had_section: bool,
    gitignore_bytes: Vec<u8>,
    gitignore_before: FileSnapshot,
}

impl PlannedFiles {
    fn plan(
        path: &Utf8Path,
        invocation: &crate::onboarding::Invocation,
        manifest_before: FileSnapshot,
        manifest_bytes: Vec<u8>,
    ) -> anyhow::Result<Self> {
        let agents_before = FileSnapshot::read(path.join("AGENTS.md").as_std_path())?;
        let without_legacy =
            crate::legacy_cleanup::project_agents(agents_before.bytes().unwrap_or_default());
        let agents_bytes = crate::onboarding::project(&without_legacy, invocation)?;
        let agents_had_section = String::from_utf8(without_legacy)
            .is_ok_and(|text| crate::onboarding::owns_section(&text));
        let gitignore_before = FileSnapshot::read(path.join(".gitignore").as_std_path())?;
        let gitignore_bytes = crate::gitignore::project_ignored(
            gitignore_before.bytes().unwrap_or_default(),
            ".provenance/cache/",
        )
        .context("failed to ignore the Provenance cache")?;
        Ok(Self {
            manifest_bytes,
            manifest_before,
            agents_bytes,
            agents_before,
            agents_had_section,
            gitignore_bytes,
            gitignore_before,
        })
    }
}

/// The one-line ending for a run that would change nothing.
fn already_ending(
    scope_ids: &[String],
    path: &Utf8Path,
    dictionary: &crate::ste_onboarding::Plan,
) -> InitEnding {
    InitEnding::already(
        format!(
            "Provenance is already set up for {} in {path}. No change.",
            scope_phrase(scope_ids)
        ),
        dictionary.warning(),
    )
}

/// Builds the new-versus-changed inventory for the printed summary, or
/// returns nothing when this run would not write one byte.
fn build_summary(
    path: &Utf8Path,
    scope_ids: &[String],
    planned: &PlannedFiles,
    skills: &InstallReport,
    dictionary: &crate::ste_onboarding::Plan,
) -> Option<InitSummary> {
    let manifest_changed =
        planned.manifest_before.bytes() != Some(planned.manifest_bytes.as_slice());
    let agents_changed = planned.agents_before.bytes() != Some(planned.agents_bytes.as_slice());
    let gitignore_changed =
        planned.gitignore_before.bytes() != Some(planned.gitignore_bytes.as_slice());
    let skills_unchanged = skills
        .files()
        .iter()
        .all(|file| file.status == FileStatus::Unchanged);
    let dictionary_change = dictionary.reference_change();
    if !manifest_changed
        && !agents_changed
        && !gitignore_changed
        && skills_unchanged
        && dictionary_change.is_none()
    {
        return None;
    }
    let mut summary = InitSummary::new(format!(
        "Initialized Provenance for {} in {path}",
        scope_phrase(scope_ids)
    ));
    if manifest_changed {
        let note = format!("manifest for {}", scope_phrase(scope_ids));
        if planned.manifest_before.bytes().is_none() {
            summary.push_new(".provenance/state", note);
        } else {
            summary.push_changed(".provenance/state/manifest.json", "updated the manifest");
        }
    }
    for file in skills.files() {
        let relative = std::path::Path::new(&file.path)
            .strip_prefix(path.as_std_path())
            .unwrap_or_else(|_| std::path::Path::new(&file.path));
        let relative = relative.display().to_string();
        match file.status {
            FileStatus::Unchanged => {}
            FileStatus::Installed => summary.push_new(relative, "added skill file"),
            FileStatus::Linked => summary.push_new(relative, "added link"),
            FileStatus::Updated => summary.push_changed(relative, "updated skill entry"),
            FileStatus::Removed => summary.push_changed(relative, "removed legacy file"),
        }
    }
    if agents_changed {
        let note = if planned.agents_had_section {
            "updated the Provenance section"
        } else {
            "added the Provenance section"
        };
        if planned.agents_before.bytes().is_none() {
            summary.push_new("AGENTS.md", note);
        } else {
            summary.push_changed("AGENTS.md", note);
        }
    }
    if gitignore_changed {
        if planned.gitignore_before.bytes().is_none() {
            summary.push_new(".gitignore", "added one line");
        } else {
            summary.push_changed(".gitignore", "added one line");
        }
    }
    if let Some(existed) = dictionary_change {
        if existed {
            summary.push_changed(
                ".provenance/state/dictionary.json",
                "updated the dictionary reference",
            );
        } else {
            summary.push_new(
                ".provenance/state/dictionary.json",
                "added the dictionary reference",
            );
        }
    }
    Some(summary)
}

impl InitPlan {
    #[rule("rule_init_apply_rolls_back_owned_changes")]
    pub(super) fn apply(self) -> anyhow::Result<InitEnding> {
        let layout = ProvenanceLayout::new(self.path.clone());
        self.planned
            .manifest_before
            .recheck(layout.manifest_path().as_std_path())?;
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
            rollback.replace(
                layout.manifest_path().as_std_path(),
                &self.planned.manifest_before,
                &self.planned.manifest_bytes,
            )?;
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
            || already_ending(&self.scope_ids, &self.path, &self.dictionary),
            |summary| InitEnding::applied(summary, self.dictionary.warning()),
        ))
    }
}

pub(super) fn scope_path_prefix(
    path: &Utf8Path,
    scope: &str,
) -> anyhow::Result<Option<Utf8PathBuf>> {
    let layout = ProvenanceLayout::new(path.to_path_buf());
    let Some(manifest) = read_manifest(&layout)? else {
        return Ok(None);
    };
    let scope_id = ScopeId::new(scope)?;
    Ok(manifest
        .scopes
        .iter()
        .find(|item| item.id == scope_id)
        .map(|item| item.path_prefix.as_path().to_path_buf()))
}

fn read_manifest(layout: &ProvenanceLayout) -> anyhow::Result<Option<Manifest>> {
    let snapshot = FileSnapshot::read(layout.manifest_path().as_std_path())?;
    snapshot.bytes().map(parse_manifest).transpose()
}

fn parse_manifest(bytes: &[u8]) -> anyhow::Result<Manifest> {
    let manifest = serde_json::from_slice::<Manifest>(bytes)?;
    provenance_core::ensure_supported_schema_version("manifest", manifest.schema_version)?;
    Ok(manifest)
}

fn update_scope(
    manifest: &mut Manifest,
    scope: Option<String>,
    path_prefix: Option<Utf8PathBuf>,
) -> anyhow::Result<()> {
    let Some(scope) = scope else {
        return Ok(());
    };
    let scope_id = ScopeId::new(scope)?;
    if let Some(existing) = manifest.scopes.iter_mut().find(|item| item.id == scope_id) {
        if let Some(path_prefix) = path_prefix {
            existing.path_prefix = RepoPathPrefix::new(path_prefix);
        }
    } else {
        manifest.scopes.push(Scope {
            id: scope_id,
            path_prefix: RepoPathPrefix::new(path_prefix.unwrap_or_else(|| Utf8PathBuf::from("."))),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[provenance_macros::verifies("rule_init_apply_rolls_back_owned_changes", examples)]
    fn apply_failure_rolls_back_every_owned_init_change() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        let plan = prepare_init(
            &repo,
            InitOptions {
                scope: Some("default".to_owned()),
                path_prefix: Some(Utf8PathBuf::from(".")),
                disposition_actor_ids: Vec::new(),
                clear_disposition_actors: false,
                ste_pdf: None,
                invocation_channel: crate::cli::InvocationChannel::Native,
                package_manager: None,
                quiet: true,
            },
        )
        .unwrap();
        for attempt in 0..100_u8 {
            std::fs::write(
                repo.join(format!(
                    ".AGENTS.md.provenance-{}-{attempt}.tmp",
                    std::process::id()
                )),
                "occupied\n",
            )
            .unwrap();
        }

        let error = plan.apply().unwrap_err();

        assert!(format!("{error:#}").contains("could not allocate tmp file"));
        for path in [
            ".provenance",
            ".agents",
            ".claude",
            "AGENTS.md",
            ".gitignore",
        ] {
            assert!(!repo.join(path).exists(), "{path} survived rollback");
        }
    }
}
