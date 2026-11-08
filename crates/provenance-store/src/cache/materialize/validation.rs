use super::{relation_rows, units};
use crate::cache::ProjectionFamily;
use crate::publication::PublicationGuard;
use crate::state_store::{GuardedStore, StateStore};
use crate::storage::{StateKey, StateView};
use camino::Utf8PathBuf;
use provenance_core::model::relations::RelationRow;
use provenance_core::{Manifest, ScopeId};
use provenance_macros::rule;

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
    store: GuardedStore<'g>,
    state_dir: Utf8PathBuf,
    view: Option<StateView>,
    pub units_hashed: u64,
}

impl<'g> UnitReader<'g> {
    /// Catch-up and rebuild read the state that the publication guard
    /// protects.
    #[rule("rule_catch_up_hashes_canonical_state_in_place")]
    pub fn new(guard: &'g PublicationGuard) -> Self {
        Self {
            guard,
            store: StateStore::under_guard(guard),
            state_dir: guard.layout().state_dir(),
            view: None,
            units_hashed: 0,
        }
    }

    /// The one view of canonical state that this pass hashes and parses:
    /// the view of the section that the guard holds, which the store of the
    /// guard reads too.
    #[rule("rule_catch_up_reads_units_from_the_view")]
    fn view(&mut self) -> anyhow::Result<&StateView> {
        if self.view.is_none() {
            let repository = self.guard.repository();
            let storage = repository.storage();
            self.view = Some(match repository.section() {
                Some(section) => section.view().read(storage.as_ref(), Clone::clone)?,
                None => storage.read_view()?,
            });
        }
        Ok(self.view.as_ref().expect("the view is read"))
    }

    /// Hashes one unit. `known` is the pair the projection holds for it;
    /// a unit whose bytes have not changed keeps that pair without a parse.
    pub fn hash(
        &mut self,
        unit: &units::Unit,
        known: Option<&units::UnitDigests>,
    ) -> anyhow::Result<units::UnitDigests> {
        self.units_hashed += 1;
        crate::test_probes::at("catch_up_unit_hashed")?;
        let state_dir = self.state_dir.clone();
        Ok(units::unit_digests(&state_dir, self.view()?, unit, known)?)
    }

    /// A global change validates every scope. Other passes validate only changed scopes.
    #[rule("rule_catch_up_validates_changed_units_only")]
    #[rule("rule_catch_up_stores_the_digest_of_parsed_bytes")]
    pub fn global(
        &mut self,
        known: Option<&units::UnitDigests>,
    ) -> anyhow::Result<(Manifest, units::UnitDigests)> {
        let digests = self.hash(&units::Unit::Global, known)?;
        crate::test_probes::at("catch_up_global_before_parse")?;
        let path = self.state_dir.join("manifest.json");
        crate::test_probes::record_read(&path);
        if known == Some(&digests) {
            let key = StateKey::new("manifest.json").expect("the manifest key is valid");
            let bytes = self
                .view()?
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("missing manifest: {path}"))?;
            return Ok((crate::state_store::manifest_from_bytes(bytes)?, digests));
        }
        let manifest = self.store.manifest()?;
        for scope in &manifest.scopes {
            validate(&self.store, &scope.id)?;
        }
        Ok((manifest, digests))
    }

    /// Parses the records of one scope from the view whose bytes gave
    /// `digests`.
    #[rule("rule_catch_up_stores_the_digest_of_parsed_bytes")]
    pub fn scope(
        &self,
        scope: &ScopeId,
        digests: units::UnitDigests,
    ) -> anyhow::Result<(ScopeRecords, units::UnitDigests)> {
        crate::test_probes::at("catch_up_before_parse")?;
        validate(&self.store, scope)?;
        let mut families = Vec::new();
        for family in ProjectionFamily::ALL {
            let (bytes, count) = family.guarded_records(&self.store, scope)?;
            families.push(FamilyRecords {
                family,
                bytes,
                count,
            });
        }
        let records = ScopeRecords {
            families,
            relations: relation_rows::scope_rows(&self.store, scope)?,
        };
        Ok((records, digests))
    }
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
