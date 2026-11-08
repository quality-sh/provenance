use super::ReviewStateSnapshot;
use crate::{
    cache::{ProjectionFamily, RecordFamily},
    publication::PublicationGuard,
    settings::Settings,
    state_store::{GuardedStore, StateStore, ViewStore},
};
use provenance_core::{Manifest, ScopeId};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

impl ReviewStateSnapshot {
    /// Reads canonical records from the repository protected by the guard.
    pub fn read(guard: &PublicationGuard) -> anyhow::Result<Self> {
        let store = StateStore::under_guard(guard);
        let (manifest, settings) = manifest_and_settings(guard, &store)?;
        Self::of_records(&store, manifest, settings)
    }

    /// Reads the manifest, the settings and one view of the canonical bytes
    /// under the guard. [`ReviewStateSnapshot::from_capture`] then parses the
    /// records after the guard is released. So a writer or a reader that
    /// waits for the guard does not also wait for the parse. When the view
    /// cannot be read, the capture holds the snapshot that
    /// [`ReviewStateSnapshot::read`] gives under the same guard.
    pub fn capture(guard: &PublicationGuard) -> anyhow::Result<ReviewStateCapture> {
        let guarded = StateStore::under_guard(guard);
        let (manifest, settings) = manifest_and_settings(guard, &guarded)?;
        let repository = guard.repository();
        let Ok(view) = repository.storage().read_view() else {
            return Self::of_records(&guarded, manifest, settings)
                .map(|snapshot| ReviewStateCapture(Captured::Read(Box::new(snapshot))));
        };
        let store = ViewStore::new(repository, Arc::new(view));
        Ok(ReviewStateCapture(Captured::View(Box::new(ViewCapture {
            store,
            manifest,
            settings,
        }))))
    }

    /// The snapshot of a capture. It gives the records that
    /// [`ReviewStateSnapshot::read`] gives under the guard of the capture.
    /// An error can have a different text: a caller that reports the error
    /// reads again with [`ReviewStateSnapshot::read`].
    pub fn from_capture(capture: ReviewStateCapture) -> anyhow::Result<Self> {
        match capture.0 {
            Captured::View(capture) => {
                let ViewCapture {
                    store,
                    manifest,
                    settings,
                } = *capture;
                let store = GuardedStore::of_view(store);
                crate::state_store::with_shared_parses(|| {
                    Self::of_records(&store, manifest, settings)
                })
            }
            Captured::Read(snapshot) => Ok(*snapshot),
        }
    }

    fn of_records(
        store: &GuardedStore<'_>,
        manifest: Manifest,
        settings: Option<Value>,
    ) -> anyhow::Result<Self> {
        let mut records = BTreeMap::new();
        for scope in &manifest.scopes {
            read_records(store, &scope.id, &mut records)?;
        }
        Ok(Self {
            records,
            manifest: serde_json::to_value(manifest)?,
            settings,
        })
    }
}

/// The canonical bytes of one snapshot, read under a guard that has ended.
pub struct ReviewStateCapture(Captured);

enum Captured {
    View(Box<ViewCapture>),
    Read(Box<ReviewStateSnapshot>),
}

struct ViewCapture {
    store: ViewStore,
    manifest: Manifest,
    settings: Option<Value>,
}

fn manifest_and_settings(
    guard: &PublicationGuard,
    store: &GuardedStore<'_>,
) -> anyhow::Result<(Manifest, Option<Value>)> {
    let manifest = store.manifest()?;
    manifest.ensure_has_scopes()?;
    Settings::load(guard.layout())?;
    let settings_path = guard.layout().provenance_dir().join("settings.json");
    let settings = match std::fs::read(settings_path) {
        Ok(bytes) => Some(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    Ok((manifest, settings))
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
