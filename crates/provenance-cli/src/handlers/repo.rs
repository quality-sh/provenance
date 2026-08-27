use crate::atomic_file::{FileRollbackJournal, FileSnapshot};
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;

pub(super) fn init(
    path: &Utf8Path,
    scope: Option<String>,
    path_prefix: Option<Utf8PathBuf>,
    disposition_actor_ids: Vec<String>,
    clear_disposition_actors: bool,
) -> anyhow::Result<()> {
    prepare_init(
        path,
        scope,
        path_prefix,
        disposition_actor_ids,
        clear_disposition_actors,
    )?
    .apply()
}

pub(super) struct InitPlan {
    path: Utf8PathBuf,
    manifest_before: FileSnapshot,
    manifest_bytes: Vec<u8>,
    skills: crate::skills::InitSkillPlan,
    agents_before: FileSnapshot,
    agents_bytes: Vec<u8>,
    gitignore_before: FileSnapshot,
    gitignore_bytes: Vec<u8>,
}

#[rule("rule_init_plans_all_project_writes")]
#[rule("rule_init_plan_rejection_preserves_targets")]
#[rule("rule_init_validates_planned_repository")]
pub(super) fn prepare_init(
    path: &Utf8Path,
    scope: Option<String>,
    path_prefix: Option<Utf8PathBuf>,
    disposition_actor_ids: Vec<String>,
    clear_disposition_actors: bool,
) -> anyhow::Result<InitPlan> {
    super::check::recover_repository_before_init(path)
        .context("failed to recover an interrupted repository publication")?;
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
    let agents_path = path.join("AGENTS.md");
    let agents_before = FileSnapshot::read(agents_path.as_std_path())?;
    let without_legacy =
        crate::legacy_cleanup::project_agents(agents_before.bytes().unwrap_or_default());
    let agents_bytes = crate::onboarding::project(&without_legacy)?;
    let gitignore_path = path.join(".gitignore");
    let gitignore_before = FileSnapshot::read(gitignore_path.as_std_path())?;
    let gitignore_bytes = crate::gitignore::project_ignored(
        gitignore_before.bytes().unwrap_or_default(),
        ".provenance/cache/",
    )
    .context("failed to ignore the Provenance cache")?;
    super::check::validate_repository_with_manifest(path, &manifest)
        .context("the planned Provenance state is not valid")?;
    Ok(InitPlan {
        path: path.to_path_buf(),
        manifest_before,
        manifest_bytes,
        skills,
        agents_before,
        agents_bytes,
        gitignore_before,
        gitignore_bytes,
    })
}

impl InitPlan {
    #[rule("rule_init_apply_rolls_back_owned_changes")]
    pub(super) fn apply(self) -> anyhow::Result<()> {
        let layout = ProvenanceLayout::new(self.path.clone());
        self.manifest_before
            .recheck(layout.manifest_path().as_std_path())?;
        self.skills.recheck()?;
        self.agents_before
            .recheck(self.path.join("AGENTS.md").as_std_path())?;
        self.gitignore_before
            .recheck(self.path.join(".gitignore").as_std_path())?;
        let mut rollback = FileRollbackJournal::within(self.path.as_std_path());
        let result = (|| -> anyhow::Result<()> {
            rollback.replace(
                layout.manifest_path().as_std_path(),
                &self.manifest_before,
                &self.manifest_bytes,
            )?;
            self.skills.apply_in(&mut rollback)?;
            let agents_path = self.path.join("AGENTS.md");
            if self.agents_before.bytes() != Some(self.agents_bytes.as_slice()) {
                rollback.replace(
                    agents_path.as_std_path(),
                    &self.agents_before,
                    &self.agents_bytes,
                )?;
            }
            let gitignore_path = self.path.join(".gitignore");
            if self.gitignore_before.bytes() != Some(self.gitignore_bytes.as_slice()) {
                rollback.replace(
                    gitignore_path.as_std_path(),
                    &self.gitignore_before,
                    &self.gitignore_bytes,
                )?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            return match rollback.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(error.context(format!(
                    "repository initialization rollback failed: {rollback:#}"
                ))),
            };
        }
        rollback.commit()
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
            Some("default".to_owned()),
            Some(Utf8PathBuf::from(".")),
            Vec::new(),
            false,
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
