use super::{family_inputs, relation_rows, units};
use crate::cache::ProjectionFamily;
use crate::publication::PublicationGuard;
use crate::state_store::{GuardedStore, StateStore};
use camino::Utf8PathBuf;
use provenance_core::model::relations::RelationRow;
use provenance_core::{Manifest, ScopeId};
use provenance_macros::rule;
use std::collections::BTreeMap;

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

    /// Reads the bytes of one unit. `known` is the pair the projection holds
    /// for it; a unit whose bytes have not changed gives `None`.
    pub fn hash(
        &mut self,
        unit: &units::Unit,
        known: Option<&units::UnitDigests>,
    ) -> anyhow::Result<Option<units::UnitBytes>> {
        self.units_hashed += 1;
        crate::test_probes::at("catch_up_unit_hashed")?;
        let bytes = units::unit_bytes(&self.state_dir, unit)?;
        Ok((known.map(|known| &known.stored) != Some(&bytes.stored)).then_some(bytes))
    }

    /// A global change validates every scope. Other passes validate only changed scopes.
    #[rule("rule_catch_up_validates_changed_units_only")]
    pub fn global(
        &mut self,
        known: Option<&units::UnitDigests>,
    ) -> anyhow::Result<(Manifest, units::UnitDigests)> {
        let unit = units::Unit::Global;
        let path = self.state_dir.join("manifest.json");
        let mut manifest_bytes = None;
        self.units_hashed += 1;
        crate::test_probes::at("catch_up_unit_hashed")?;
        let digests = units::digests_with(&self.state_dir, &unit, known, |read_path, bytes| {
            if read_path == path {
                manifest_bytes = Some(bytes.to_vec());
            }
        })?;
        crate::test_probes::at("catch_up_global_before_parse")?;
        crate::test_probes::record_read(&path);
        if known == Some(&digests) {
            let bytes =
                manifest_bytes.ok_or_else(|| anyhow::anyhow!("missing manifest: {path}"))?;
            return Ok((crate::state_store::manifest_from_bytes(&bytes)?, digests));
        }
        let state_dir = &self.state_dir;
        stable(
            &unit,
            digests,
            |digests| Ok(units::unit_digests(state_dir, &unit, Some(digests))?),
            |_, _| {
                let manifest = self.store.manifest()?;
                for scope in &manifest.scopes {
                    validate(&self.store, &scope.id)?;
                }
                Ok(manifest)
            },
        )
    }

    /// Validates the whole scope and parses the families that the changed
    /// files of `read` can change, or every family when `all` is set. The
    /// relation rows are derived when an owner family is parsed.
    pub fn scope(
        &self,
        scope: &ScopeId,
        read: units::UnitRead,
        all: bool,
    ) -> anyhow::Result<(ScopeRecords, units::UnitRead)> {
        let unit = units::Unit::Scope(scope.clone());
        let state_dir = &self.state_dir;
        stable(
            &unit,
            read,
            |read| {
                let after = units::unit_digests(state_dir, &unit, Some(&read.digests))?;
                if after == read.digests {
                    return Ok(read.clone());
                }
                let bytes = units::unit_bytes(state_dir, &unit)?;
                Ok(units::files_read(&bytes, &BTreeMap::new())?)
            },
            |read, retry| {
                crate::state_store::with_shared_parses(|| {
                    self.parse_scope(scope, read, all || retry)
                })
            },
        )
    }

    /// Validates the scope and parses the selected families, or every family
    /// when `all` is set. See [`Self::scope`].
    fn parse_scope(
        &self,
        scope: &ScopeId,
        read: &units::UnitRead,
        all: bool,
    ) -> anyhow::Result<ScopeRecords> {
        crate::test_probes::at("catch_up_before_parse")?;
        validate(&self.store, scope)?;
        let selected = if all {
            None
        } else {
            family_inputs::affected(scope, &read.changed)
        };
        let selected = selected.unwrap_or_else(|| ProjectionFamily::ALL.to_vec());
        let mut families = Vec::new();
        for family in selected {
            let (bytes, count) = family.records_of_validated_scope(&self.store, scope)?;
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
                relation_rows::scope_rows(&self.store, scope)?
            } else {
                Vec::new()
            },
        })
    }
}

fn validate(store: &GuardedStore<'_>, scope: &ScopeId) -> anyhow::Result<()> {
    store.validate_ideation_scope(scope)?;
    store.validate_graph_scope(scope)
}

/// A parsed unit is used only when its hashes before and after parsing
/// match. After a change, the next parse gets `true` and parses every family.
#[rule("rule_catch_up_stores_the_digest_of_parsed_bytes")]
fn stable<T, D: PartialEq>(
    unit: &units::Unit,
    mut digests: D,
    mut rehash: impl FnMut(&D) -> anyhow::Result<D>,
    mut parse: impl FnMut(&D, bool) -> anyhow::Result<T>,
) -> anyhow::Result<(T, D)> {
    let mut retry = false;
    for _ in 0..3 {
        let records = parse(&digests, retry);
        let after = rehash(&digests)?;
        if after == digests {
            return records.map(|records| (records, digests));
        }
        digests = after;
        retry = true;
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
