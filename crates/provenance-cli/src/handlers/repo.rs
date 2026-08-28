use crate::atomic_file::{FileRollbackJournal, FileSnapshot};
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Capability, Manifest, RepoPathPrefix, Scope, ScopeId};
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;

pub(super) const DISPOSITION_ACTOR_DEPRECATION: &str =
    "warning: init --disposition-actor-id / --clear-disposition-actors are deprecated inside the \
     rbac compatibility window and are removed at the next protocol bump; move disposition \
     authority into rbac.assignments in .provenance/state/manifest.json";

pub(super) struct InitOptions {
    pub(super) scope: Option<String>,
    pub(super) path_prefix: Option<Utf8PathBuf>,
    pub(super) disposition_actor_ids: Vec<String>,
    pub(super) clear_disposition_actors: bool,
    pub(super) actor_claim: Option<provenance_core::RbacClaim>,
    pub(super) ste_onboarding: crate::cli::SteOnboardingMode,
    pub(super) ste_pdf: Option<Utf8PathBuf>,
    pub(super) invocation_channel: crate::cli::InvocationChannel,
    pub(super) package_manager: Option<crate::cli::PackageManager>,
}

pub(super) fn init(path: &Utf8Path, options: InitOptions) -> anyhow::Result<()> {
    if !options.disposition_actor_ids.is_empty() || options.clear_disposition_actors {
        eprintln!("{DISPOSITION_ACTOR_DEPRECATION}");
    }
    prepare_init(path, options)?.apply()
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
    dictionary: crate::ste_onboarding::Plan,
    actor_claim: Option<provenance_core::RbacClaim>,
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
        actor_claim,
        ste_onboarding,
        ste_pdf,
        invocation_channel,
        package_manager,
    } = options;
    let actor_claim = actor_claim.as_ref();
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
    let agents_path = path.join("AGENTS.md");
    let agents_before = FileSnapshot::read(agents_path.as_std_path())?;
    let without_legacy =
        crate::legacy_cleanup::project_agents(agents_before.bytes().unwrap_or_default());
    let agents_bytes = crate::onboarding::project(&without_legacy, &invocation)?;
    let gitignore_path = path.join(".gitignore");
    let gitignore_before = FileSnapshot::read(gitignore_path.as_std_path())?;
    let gitignore_bytes = crate::gitignore::project_ignored(
        gitignore_before.bytes().unwrap_or_default(),
        ".provenance/cache/",
    )
    .context("failed to ignore the Provenance cache")?;
    super::check::validate_repository_with_manifest(path, &manifest)
        .context("the planned Provenance state is not valid")?;
    let dictionary = crate::ste_onboarding::prepare(path, ste_onboarding, ste_pdf.as_deref())?;
    Ok(InitPlan {
        path: path.to_path_buf(),
        manifest_before,
        manifest_bytes,
        skills,
        agents_before,
        agents_bytes,
        gitignore_before,
        gitignore_bytes,
        dictionary,
        actor_claim: actor_claim.cloned(),
    })
}

impl InitPlan {
    /// Applies the plan. A re-init — any repository that already has a
    /// manifest — decides and writes inside one publication-critical section:
    /// the manifest-write decision resolves against the manifest bytes the
    /// recheck just confirmed current, and the protected writes happen in the
    /// same section, so no concurrent writer can move the manifest between
    /// the decision and the bytes (census row 20). First bootstrap is exempt:
    /// the rbac section it creates cannot be consulted before it exists, and
    /// no publication lock directory may precede the repository it serves.
    #[rule("rule_init_apply_rolls_back_owned_changes")]
    pub(super) fn apply(self) -> anyhow::Result<()> {
        let layout = ProvenanceLayout::new(self.path.clone());
        let reinit = self.manifest_before.bytes().is_some();
        if reinit {
            provenance_store::publication::with_repository_publication(&layout, || {
                self.apply_prepared(&layout, true)
            })
        } else {
            self.apply_prepared(&layout, false)
        }
    }

    fn apply_prepared(self, layout: &ProvenanceLayout, reinit: bool) -> anyhow::Result<()> {
        self.manifest_before
            .recheck(layout.manifest_path().as_std_path())?;
        self.skills.recheck()?;
        self.agents_before
            .recheck(self.path.join("AGENTS.md").as_std_path())?;
        self.gitignore_before
            .recheck(self.path.join(".gitignore").as_std_path())?;
        self.dictionary.recheck(&self.path)?;
        if reinit {
            self.authorize_reinit()?;
        }
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
            self.dictionary.apply_in(&self.path, &mut rollback)?;
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
        rollback.commit()?;
        self.dictionary.print_message();
        Ok(())
    }

    /// The re-init decision, made inside the publication-critical section
    /// against the snapshot bytes the recheck above tied to the live file:
    /// on an rbac-managed repository the claim must hold `manifest-write` on
    /// every scope then listed (settled Option A).
    fn authorize_reinit(&self) -> anyhow::Result<()> {
        let bytes = self
            .manifest_before
            .bytes()
            .ok_or_else(|| anyhow::anyhow!("re-init requires an existing manifest"))?;
        let manifest = parse_manifest(bytes)?;
        let Some(section) = &manifest.rbac else {
            return Ok(());
        };
        let scopes: Vec<ScopeId> = manifest.scopes.iter().map(|s| s.id.clone()).collect();
        provenance_core::authorize(
            self.actor_claim.as_ref(),
            section,
            Capability::ManifestWrite,
            provenance_core::RbacResource::RepoGlobal(&scopes),
        )
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
    provenance_core::ensure_manifest_rbac_laws(
        &manifest.disposition_actor_ids,
        manifest.rbac.as_ref(),
    )?;
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
                actor_claim: None,
                ste_onboarding: crate::cli::SteOnboardingMode::Interactive,
                ste_pdf: None,
                invocation_channel: crate::cli::InvocationChannel::Native,
                package_manager: None,
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

    fn reinit_options() -> InitOptions {
        InitOptions {
            scope: None,
            path_prefix: None,
            disposition_actor_ids: Vec::new(),
            clear_disposition_actors: false,
            actor_claim: None,
            ste_onboarding: crate::cli::SteOnboardingMode::Interactive,
            ste_pdf: None,
            invocation_channel: crate::cli::InvocationChannel::Native,
            package_manager: None,
        }
    }

    fn bootstrap(directory: &std::path::Path) -> anyhow::Result<(Utf8PathBuf, ProvenanceLayout)> {
        let repo = Utf8PathBuf::from_path_buf(directory.join("repo")).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        prepare_init(
            &repo,
            InitOptions {
                scope: Some("default".to_owned()),
                path_prefix: Some(Utf8PathBuf::from(".")),
                ..reinit_options()
            },
        )?
        .apply()?;
        let layout = ProvenanceLayout::new(repo.clone());
        Ok((repo, layout))
    }

    #[test]
    fn reinit_applies_inside_the_publication_critical_section() {
        let directory = tempfile::tempdir().unwrap();
        let (repo, layout) = bootstrap(directory.path()).unwrap();

        let plan = prepare_init(&repo, reinit_options()).unwrap();

        let (acquired_tx, acquired_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let store = provenance_store::state_store::StateStore::new(layout);
        let holder = std::thread::spawn(move || {
            store
                .with_repository_publication(|| {
                    acquired_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                })
                .unwrap();
        });
        acquired_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the holder acquired the publication lock");

        let reinit = std::thread::spawn(move || plan.apply());
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            !reinit.is_finished(),
            "re-init must decide and write inside the publication-critical section, \
             not before or beside it"
        );

        release_tx.send(()).unwrap();
        holder.join().unwrap();
        reinit.join().unwrap().expect("the re-init completes");
    }
}
