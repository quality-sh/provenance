use super::{relation_rows, units};
use crate::cache::ProjectionFamily;
use crate::publication::PublicationGuard;
use crate::state_store::{GuardedStore, StateStore};
use camino::{Utf8Path, Utf8PathBuf};
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
    store: GuardedStore<'g>,
    state_dir: Utf8PathBuf,
    pub units_hashed: u64,
}

impl<'g> UnitReader<'g> {
    /// Catch-up and rebuild read the tree protected by the publication guard.
    #[rule("rule_catch_up_hashes_canonical_state_in_place")]
    pub fn new(guard: &'g PublicationGuard) -> Self {
        Self {
            store: StateStore::under_guard(guard),
            state_dir: guard.layout().state_dir(),
            units_hashed: 0,
        }
    }

    pub fn hash(&mut self, unit: &units::Unit) -> anyhow::Result<String> {
        hash(&self.state_dir, unit, &mut self.units_hashed)
    }

    /// A global change validates every scope. Other passes validate only changed scopes.
    #[rule("rule_catch_up_validates_changed_units_only")]
    pub fn global(&mut self, stored: Option<&String>) -> anyhow::Result<(Manifest, String)> {
        let unit = units::Unit::Global;
        let path = self.state_dir.join("manifest.json");
        let mut manifest_bytes = None;
        self.units_hashed += 1;
        crate::test_probes::at("catch_up_unit_hashed")?;
        let digest = units::digest_with(&self.state_dir, &unit, |read_path, bytes| {
            if read_path == path {
                manifest_bytes = Some(bytes.to_vec());
            }
        })?;
        crate::test_probes::at("catch_up_global_before_parse")?;
        crate::test_probes::record_read(&path);
        if stored == Some(&digest) {
            let bytes =
                manifest_bytes.ok_or_else(|| anyhow::anyhow!("missing manifest: {path}"))?;
            return Ok((crate::state_store::manifest_from_bytes(&bytes)?, digest));
        }
        stable(&self.state_dir, &unit, digest, || {
            let manifest = self.store.manifest()?;
            for scope in &manifest.scopes {
                validate(&self.store, &scope.id)?;
            }
            Ok(manifest)
        })
    }

    pub fn scope(&self, scope: &ScopeId, digest: String) -> anyhow::Result<(ScopeRecords, String)> {
        stable(
            &self.state_dir,
            &units::Unit::Scope(scope.clone()),
            digest,
            || {
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
                Ok(ScopeRecords {
                    families,
                    relations: relation_rows::scope_rows(&self.store, scope)?,
                })
            },
        )
    }
}

fn validate(store: &GuardedStore<'_>, scope: &ScopeId) -> anyhow::Result<()> {
    store.validate_ideation_scope(scope)?;
    store.validate_graph_scope(scope)
}

fn hash(state_dir: &Utf8Path, unit: &units::Unit, hashes: &mut u64) -> anyhow::Result<String> {
    *hashes += 1;
    crate::test_probes::at("catch_up_unit_hashed")?;
    Ok(units::unit_digest(state_dir, unit)?)
}

/// A parsed unit is used only when its hashes before and after parsing match.
#[rule("rule_catch_up_stores_the_digest_of_parsed_bytes")]
fn stable<T>(
    state_dir: &Utf8Path,
    unit: &units::Unit,
    mut digest: String,
    mut parse: impl FnMut() -> anyhow::Result<T>,
) -> anyhow::Result<(T, String)> {
    for _ in 0..3 {
        let records = parse();
        let after = units::unit_digest(state_dir, unit)?;
        if after == digest {
            return records.map(|records| (records, digest));
        }
        digest = after;
    }
    anyhow::bail!(
        "canonical state changed during catch-up under {}",
        unit.name()
    )
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
