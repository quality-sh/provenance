use std::path::PathBuf;

use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_macros::rule;
use provenance_ste100::{DictionaryImport, DictionaryImportIdentity};

use crate::layout::ProvenanceLayout;

pub fn dictionary_reference_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.state_dir().join("dictionary.json")
}

/// Loads the referenced dictionary, or nothing when it cannot load.
#[rule("rule_ste_dictionary_reference_resolution")]
pub fn load_project_dictionary(layout: &ProvenanceLayout) -> Option<DictionaryImport> {
    let reference = std::fs::read(dictionary_reference_path(layout).as_std_path()).ok()?;
    let identity: DictionaryImportIdentity = serde_json::from_slice(&reference).ok()?;
    let directory = index_directory()?;
    provenance_ste100::load_dictionary_index(&directory, &identity).ok()
}

/// Stores the index in the machine data directory and writes the project reference.
///
/// This write bypasses both mutation primitives (it is a direct write of
/// `state/dictionary.json`), so it carries its own named gate. The whole
/// operation — authorization and both writes — runs inside the publication
/// lock: the shared policy choke resolves the claim against the canonical
/// manifest bytes no concurrent writer can move, and only then do bytes move.
/// On an rbac-managed repository the claim must hold `edit` on every scope
/// then listed — the settled Option A rule for repo-global resources
/// (census row 19). A repository with no manifest is not rbac-managed.
pub fn set_project_dictionary(
    layout: &ProvenanceLayout,
    claim: Option<&provenance_core::RbacClaim>,
    import: &DictionaryImport,
) -> anyhow::Result<Utf8PathBuf> {
    use std::io::Write;
    crate::publication::with_repository_publication(layout, || {
        if layout.manifest_path().exists() {
            crate::state_store::StateStore::new(layout.clone())
                .ensure_repo_global_mutation(claim, provenance_core::Capability::Edit)?;
        }
        let directory = index_directory().context("no machine data directory is available")?;
        provenance_ste100::store_dictionary_index(import, &directory)
            .context("store the dictionary index")?;
        let path = dictionary_reference_path(layout);
        std::fs::create_dir_all(layout.state_dir().as_std_path())
            .context("create the project state directory")?;
        let mut reference = serde_json::to_vec_pretty(&import.identity)
            .context("serialize the dictionary reference")?;
        reference.push(b'\n');
        let mut staged = tempfile::NamedTempFile::new_in(layout.state_dir().as_std_path())?;
        staged.write_all(&reference)?;
        staged.persist(path.as_std_path())?;
        Ok(path)
    })
}

/// The machine directory that holds imported dictionary index files.
pub fn index_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("PROVENANCE_STE100_INDEX_DIR") {
        return Some(PathBuf::from(directory));
    }
    Some(
        data_directory()?
            .join("provenance")
            .join("ste100-dictionary"),
    )
}

#[cfg(target_os = "windows")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_store::StateStore;
    use std::sync::mpsc;
    use std::time::Duration;

    fn fixture() -> (tempfile::TempDir, ProvenanceLayout, DictionaryImport) {
        let directory = tempfile::tempdir().unwrap();
        let layout = ProvenanceLayout::new(
            Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap(),
        );
        std::fs::create_dir_all(layout.state_dir().as_std_path()).unwrap();
        // Keep the index out of the machine data directory while testing.
        std::env::set_var(
            "PROVENANCE_STE100_INDEX_DIR",
            directory.path().join("index"),
        );
        let import = DictionaryImport {
            identity: DictionaryImportIdentity {
                issue: provenance_ste100::StandardIssue::Nine,
                source_sha256: "0".repeat(64),
                data_sha256: "1".repeat(64),
                extractor_version: "test".to_owned(),
            },
            entries: Vec::new(),
        };
        (directory, layout, import)
    }

    fn write_manifest(layout: &ProvenanceLayout, body: &str) {
        std::fs::write(layout.manifest_path().as_std_path(), body).unwrap();
    }

    fn grants(legacy_actors: &str, assignments: &str) -> String {
        format!(
            r#"{{
            "schema_version": 1,
            "scopes": [{{"id": "default", "path_prefix": "."}}],
            "disposition_actor_ids": {legacy_actors},
            "rbac": {assignments}
        }}"#
        )
    }

    fn reviewer(assignments: &str) -> String {
        format!(r#"{{"assignments": [{assignments}]}}"#)
    }

    fn claim(actor: &str) -> provenance_core::RbacClaim {
        provenance_core::RbacClaim::new(actor).unwrap()
    }

    #[test]
    fn a_dictionary_write_waits_for_the_publication_lock() {
        let (directory, layout, import) = fixture();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let store = StateStore::new(layout.clone());
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
            .recv_timeout(Duration::from_secs(10))
            .expect("the holder acquired the publication lock");

        let writer = std::thread::spawn(move || set_project_dictionary(&layout, None, &import));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            !writer.is_finished(),
            "the dictionary write must wait for the publication lock, not bypass it"
        );

        release_tx.send(()).unwrap();
        holder.join().unwrap();
        writer
            .join()
            .unwrap()
            .expect("the write completes once the lock frees");
        std::mem::drop(directory);
    }

    #[test]
    fn an_ambiguous_manifest_refuses_the_dictionary_write() {
        let (_directory, layout, import) = fixture();
        write_manifest(
            &layout,
            &grants(
                r#"["ben"]"#,
                &reviewer(
                    r#"{"actor_id": "reviewer", "identity_type": "human", "capabilities": ["edit"], "scopes": ["default"]}"#,
                ),
            ),
        );

        let error = set_project_dictionary(&layout, Some(&claim("reviewer")), &import).unwrap_err();

        assert!(
            format!("{error:#}").contains(
                "ambiguous manifest: disposition_actor_ids and rbac.assignments are both present"
            ),
            "the canonical manifest read law must gate the dictionary write: {error:#}"
        );
    }

    #[test]
    fn a_dictionary_write_on_an_rbac_repository_demands_edit_on_every_scope() {
        let (_directory, layout, import) = fixture();
        write_manifest(
            &layout,
            &grants(
                "[]",
                &reviewer(
                    r#"{"actor_id": "reviewer", "identity_type": "human", "capabilities": ["edit"], "scopes": ["default"]}"#,
                ),
            ),
        );

        let error = set_project_dictionary(&layout, Some(&claim("intruder")), &import).unwrap_err();
        assert!(
            format!("{error:#}")
                .contains("rbac: actor intruder does not hold capability edit on scope default"),
            "{error:#}"
        );

        let error = set_project_dictionary(&layout, None, &import).unwrap_err();
        assert!(
            format!("{error:#}").contains("rbac: no actor claim supplied for a mutating operation"),
            "{error:#}"
        );

        set_project_dictionary(&layout, Some(&claim("reviewer")), &import)
            .expect("a granted principal writes the dictionary reference");
        assert!(dictionary_reference_path(&layout).exists());
    }
}
