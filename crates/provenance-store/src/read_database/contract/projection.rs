//! Cases on the projection that the engine keeps in the read database.
//!
//! The catch-up and the schema check run as the engine runs them, over a
//! repository whose state is in memory and whose read database is the
//! adapter under test.
//!
//! The cases build with `host` only: each pass takes the publication
//! guard, whose wait for the section lock runs on the blocking pool.

use super::{fetch, statement, write, Harness};
use crate::cache::{catch_up_state, materialize_state, CatchUpReport};
use crate::read_database::{ReadDatabase, SqlValue};
use crate::repository::{Repository, RepositoryIdentity};
use crate::settings::Settings;
use crate::storage::memory::{MemorySectionLock, MemoryStateStorage};
use crate::storage::StateKey;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use std::collections::BTreeMap;
use std::sync::Arc;

fn key(value: &str) -> StateKey {
    StateKey::new(value).unwrap()
}

fn requirement(id: &str) -> serde_json::Value {
    serde_json::json!({
        "scope_id": "default",
        "id": id,
        "statement": "Share links expire",
        "status": "active",
    })
}

/// A repository whose state is the manifest and one Requirement, held in
/// memory, over a second adapter of the harness's store.
fn repository<H: Harness>(harness: &H) -> Repository {
    let scope = ScopeId::new("default").unwrap();
    let manifest = Manifest::default_with_scope(scope.clone(), RepoPathPrefix::new("."));
    let mut shard = serde_json::to_vec(&requirement("req_first")).unwrap();
    shard.push(b'\n');
    let entries = BTreeMap::from([
        (key("manifest.json"), serde_json::to_vec(&manifest).unwrap()),
        (key("scopes/default/requirements/req.jsonl"), shard),
    ]);
    let database: Arc<dyn ReadDatabase> = Arc::from(harness.reopen());
    Repository::with_storage(
        RepositoryIdentity::new("/provenance-contract".into(), scope, Settings::default()),
        Arc::new(MemoryStateStorage::new(entries)),
        Arc::new(MemorySectionLock::default()),
        Some(database),
    )
    .unwrap()
}

/// The serial, and whether the pass rebuilt, committed and made the schema
/// again.
const fn outcome(report: &CatchUpReport) -> (i64, bool, bool, bool) {
    (
        report.serial,
        report.rebuilt,
        report.revision_committed,
        report.cache_recreated,
    )
}

async fn requirement_ids(database: &dyn ReadDatabase) -> Vec<SqlValue> {
    fetch(database, "SELECT id FROM requirements ORDER BY id")
        .await
        .into_iter()
        .flatten()
        .collect()
}

/// A catch-up makes the projection and then follows the state.
///
/// A catch-up over an empty store makes the schema and the first revision.
/// A second pass with no change commits nothing, and a pass after a
/// publish commits the next revision with the published rows.
pub async fn a_catch_up_makes_and_follows_the_projection<H: Harness>(harness: H) {
    let repository = repository(&harness);
    let first = catch_up_state(&repository).await.unwrap();
    assert_eq!(outcome(&first), (1, true, true, true));
    let unchanged = catch_up_state(&repository).await.unwrap();
    assert_eq!(outcome(&unchanged), (1, false, false, false));
    assert_eq!(unchanged.digest, first.digest);

    repository
        .store()
        .create_requirement(
            serde_json::from_value(serde_json::json!({
                "scope_id": "default",
                "id": "req_second",
                "statement": "Share links expire",
                "status": "active",
                "depends_on": [],
                "supersedes": [],
            }))
            .unwrap(),
        )
        .unwrap();
    let next = catch_up_state(&repository).await.unwrap();
    assert_eq!(outcome(&next), (2, false, true, false));
    assert_ne!(next.digest, first.digest);
    assert_eq!(
        requirement_ids(harness.database()).await,
        vec![SqlValue::from("req_first"), SqlValue::from("req_second")]
    );
}

/// A schema that is not current is made again.
///
/// The schema check finds a missing index, a missing table, a metadata row
/// with another digest, and no metadata table, and the catch-up then makes
/// the schema again. A rebuild over a current schema keeps it.
pub async fn a_schema_that_is_not_current_is_made_again<H: Harness>(harness: H) {
    let repository = repository(&harness);
    let first = catch_up_state(&repository).await.unwrap();
    for change in [
        "DROP INDEX idx_relations_in",
        "DROP TABLE relations",
        "UPDATE _cache_metadata SET schema_digest = 'another'",
        "DROP TABLE _cache_metadata",
    ] {
        write(harness.database(), vec![statement(change, Vec::new())])
            .await
            .unwrap();
        // A schema made again starts the serials again.
        let pass = catch_up_state(&repository).await.unwrap();
        assert_eq!(outcome(&pass), (1, true, true, true), "{change}");
        assert_eq!(pass.digest, first.digest, "{change}");
        let unchanged = catch_up_state(&repository).await.unwrap();
        assert_eq!(outcome(&unchanged), (1, false, false, false), "{change}");
    }
    let rebuilt = materialize_state(&repository).await.unwrap();
    assert_eq!(
        (rebuilt.records_loaded, rebuilt.cache_recreated),
        (1, false)
    );
    let after = catch_up_state(&repository).await.unwrap();
    assert_eq!(outcome(&after), (2, false, false, false));
    assert_eq!(
        requirement_ids(harness.database()).await,
        vec![SqlValue::from("req_first")]
    );
}
