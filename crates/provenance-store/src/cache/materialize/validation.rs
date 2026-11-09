use super::{family_inputs, relation_rows, units};
use crate::cache::ProjectionFamily;
use crate::publication::PublicationGuard;
use crate::state_store::{FencedStore, GuardedStore, SharedParses, ViewStore};
use crate::storage::{StateKey, StateView};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::model::relations::RelationRow;
use provenance_core::{Manifest, ScopeId};
use provenance_macros::rule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub(super) struct FamilyRecords {
    pub family: ProjectionFamily,
    pub bytes: Vec<u8>,
    pub count: u64,
}

pub(super) struct ScopeRecords {
    pub families: Vec<FamilyRecords>,
    pub relations: Vec<RelationRow>,
}

pub(super) struct UnitReader<'g> {
    guard: &'g PublicationGuard,
    state_dir: Utf8PathBuf,
    pass: Option<PassView>,
    /// The catch-up holds the reader across an await of a future that
    /// must be `Send`, so the count is atomic.
    units_hashed: AtomicU64,
}

/// The one view of canonical state that a pass hashes and parses, and the
/// store that reads it.
struct PassView {
    view: Arc<StateView>,
    store: GuardedStore<'static>,
}

impl<'g> UnitReader<'g> {
    /// Catch-up and rebuild read the state that the publication guard
    /// protects.
    #[rule("rule_catch_up_hashes_canonical_state_in_place")]
    pub fn new(guard: &'g PublicationGuard) -> Self {
        Self {
            guard,
            state_dir: guard.layout().state_dir(),
            pass: None,
            units_hashed: AtomicU64::new(0),
        }
    }

    /// The state directory whose view this pass reads.
    pub fn state_dir(&self) -> &Utf8Path {
        &self.state_dir
    }

    /// The number of units that this pass hashed.
    pub fn units_hashed(&self) -> u64 {
        self.units_hashed.load(Ordering::Relaxed)
    }

    /// Reads the one view of canonical state that this pass hashes and
    /// parses: the view of the section that the guard holds, or a view that
    /// the adapter reads under the guard. The store of the pass reads this
    /// view too. So the digests that a pass stores are the digests of the
    /// bytes that it parsed.
    #[rule("rule_catch_up_reads_units_from_the_view")]
    #[rule("rule_catch_up_stores_the_digest_of_parsed_bytes")]
    fn read_view(&mut self) -> anyhow::Result<&PassView> {
        if self.pass.is_none() {
            let repository = self.guard.repository();
            let storage = repository.storage();
            let view = Arc::new(match repository.section() {
                Some(section) => section.view().read(storage.as_ref(), Clone::clone)?,
                None => storage.read_view()?,
            });
            let store = GuardedStore::of_view(ViewStore::new(repository, Arc::clone(&view)));
            self.pass = Some(PassView { view, store });
        }
        Ok(self.pass.as_ref().expect("the view is read"))
    }

    /// The view of this pass. [`Self::global`] reads it first.
    fn pass(&self) -> anyhow::Result<&PassView> {
        self.pass
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("the catch-up read no view before it hashed a scope"))
    }

    /// The bytes of one unit in the view. `known` is the pair the
    /// projection holds for it; a unit whose bytes have not changed gives
    /// `None`.
    pub fn hash(
        &self,
        unit: &units::Unit,
        known: Option<&units::UnitDigests>,
    ) -> anyhow::Result<Option<units::UnitBytes<'_>>> {
        self.units_hashed.fetch_add(1, Ordering::Relaxed);
        crate::test_probes::at("catch_up_unit_hashed")?;
        let bytes = units::UnitBytes::of_view(&self.pass()?.view, unit);
        Ok((known.map(|known| &known.stored) != Some(&bytes.stored)).then_some(bytes))
    }

    /// A global change validates every scope. Other passes validate only changed scopes.
    #[rule("rule_catch_up_validates_changed_units_only")]
    pub fn global(
        &mut self,
        known: Option<&units::UnitDigests>,
    ) -> anyhow::Result<(Manifest, units::UnitDigests)> {
        self.units_hashed.fetch_add(1, Ordering::Relaxed);
        crate::test_probes::at("catch_up_unit_hashed")?;
        let state_dir = self.state_dir.clone();
        let pass = self.read_view()?;
        let digests = units::unit_digests(&state_dir, &pass.view, &units::Unit::Global, known)?;
        crate::test_probes::at("catch_up_global_before_parse")?;
        let path = state_dir.join("manifest.json");
        crate::test_probes::record_read(&path);
        if known == Some(&digests) {
            let key = StateKey::new("manifest.json").expect("the manifest key is valid");
            let bytes = pass
                .view
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("missing manifest: {path}"))?;
            return Ok((crate::state_store::manifest_from_bytes(bytes)?, digests));
        }
        let manifest = pass.store.manifest()?;
        for scope in &manifest.scopes {
            validate(&pass.store, &scope.id)?;
        }
        Ok((manifest, digests))
    }

    /// Validates the whole scope and parses the families that the changed
    /// files of `read` can change, or every family when `all` is set. The
    /// relation rows are derived when an owner family is parsed. The pass
    /// parses the view whose bytes gave `read`. The stores of the scope
    /// share their parses of that view.
    pub fn scope(
        &self,
        scope: &ScopeId,
        read: &units::UnitRead,
        all: bool,
    ) -> anyhow::Result<ScopeRecords> {
        crate::test_probes::at("catch_up_before_parse")?;
        let parses = SharedParses::new(Arc::clone(&self.pass()?.view));
        let repository = self.guard.repository();
        validate(
            &GuardedStore::of_view(ViewStore::sharing(repository, &parses)),
            scope,
        )?;
        let selected = if all {
            None
        } else {
            family_inputs::affected(scope, &read.changed)
        };
        let selected = selected.unwrap_or_else(|| ProjectionFamily::ALL.to_vec());
        let mut families = Vec::new();
        for family in selected {
            let fenced = fenced(
                repository,
                &parses,
                scope,
                family_inputs::input_directories(family),
            );
            let (bytes, count) = family.records_of_validated_scope(fenced, scope)?;
            families.push(FamilyRecords {
                family,
                bytes,
                count,
            });
        }
        let owners = families
            .iter()
            .any(|records| relation_rows::OWNERS.contains(&records.family));
        Ok(ScopeRecords {
            families,
            relations: if owners {
                let fenced = fenced(
                    repository,
                    &parses,
                    scope,
                    relation_rows::input_directories(),
                );
                relation_rows::scope_rows(fenced, scope)?
            } else {
                Vec::new()
            },
        })
    }
}

/// A store over the view of `parses` that shares them and reads only
/// `directories` below the scope directory of `scope`.
fn fenced(
    repository: &crate::repository::Repository,
    parses: &Arc<SharedParses>,
    scope: &ScopeId,
    directories: Vec<&'static str>,
) -> FencedStore {
    ViewStore::sharing(repository, parses).fenced(scope, directories)
}

fn validate(store: &GuardedStore<'_>, scope: &ScopeId) -> anyhow::Result<()> {
    store.validate_ideation_scope(scope)?;
    store.validate_graph_scope(scope)
}

/// A projection from a different validator version requires a full rebuild.
#[rule("rule_validation_version_move_rebuilds_the_projection")]
pub(super) async fn version_changed(pool: &sqlx::SqlitePool) -> anyhow::Result<bool> {
    let version: Option<i64> =
        sqlx::query_scalar("SELECT version FROM projection_validation WHERE only_row = 1")
            .fetch_optional(pool)
            .await?;
    Ok(version != Some(i64::from(crate::VALIDATION_VERSION)))
}
