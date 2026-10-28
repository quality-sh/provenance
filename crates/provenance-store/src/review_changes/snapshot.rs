use super::ReviewStateSnapshot;
use crate::{
    cache::{ProjectionFamily, RecordFamily},
    publication::PublicationGuard,
    settings::Settings,
    state_store::{GuardedStore, StateStore},
};
use provenance_core::ScopeId;
use serde_json::Value;
use std::collections::BTreeMap;

impl ReviewStateSnapshot {
    /// Reads canonical records from the repository protected by the guard.
    pub fn read(guard: &PublicationGuard) -> anyhow::Result<Self> {
        let store = StateStore::under_guard(guard);
        let manifest = store.manifest()?;
        manifest.ensure_has_scopes()?;
        Settings::load(guard.layout())?;
        let settings_path = guard.layout().provenance_dir().join("settings.json");
        let settings = match std::fs::read(settings_path) {
            Ok(bytes) => Some(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut records = BTreeMap::new();
        for scope in &manifest.scopes {
            read_records(&store, &scope.id, &mut records)?;
        }
        Ok(Self {
            records,
            manifest: serde_json::to_value(manifest)?,
            settings,
        })
    }
}

fn add_records<K: RecordFamily>(
    store: &GuardedStore<'_>,
    scope: &ScopeId,
    family: ProjectionFamily,
    records: &mut BTreeMap<(String, String, String), Value>,
) -> anyhow::Result<()> {
    let kind = family.meta().node_type.map_or_else(
        || family.family_name().to_owned(),
        |kind| kind.as_str().to_owned(),
    );
    for record in K::guarded(store, scope)? {
        let key = (
            scope.as_str().to_owned(),
            kind.clone(),
            record.record_id().to_owned(),
        );
        records.insert(key, serde_json::to_value(record)?);
    }
    Ok(())
}

macro_rules! snapshot_families {
    ($( $group:ident { $( $variant:ident {
        record: $record:ty, $($rest:tt)*
    }; )* } )*) => {
        fn read_records(
            store: &GuardedStore<'_>,
            scope: &ScopeId,
            records: &mut BTreeMap<(String, String, String), Value>,
        ) -> anyhow::Result<()> {
            $($(add_records::<$record>(store, scope, ProjectionFamily::$variant, records)?;)*)*
            Ok(())
        }
    };
}

crate::cache::record_families!(snapshot_families);
