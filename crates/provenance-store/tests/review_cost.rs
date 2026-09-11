use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::{
    layout::ProvenanceLayout, publication::with_staged_state, state_store::StateStore,
};
use serde_json::json;
use std::time::Instant;

fn copy_tree(source: &Utf8Path, destination: &Utf8Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let path = Utf8PathBuf::from_path_buf(entry.path()).unwrap();
        let target = destination.join(path.file_name().unwrap());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(path, target).unwrap();
        }
    }
}

fn size(path: &Utf8Path) -> (u64, u64) {
    let mut result = (0, 0);
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();
        if meta.is_dir() {
            let child = size(Utf8Path::from_path(&entry.path()).unwrap());
            result.0 += child.0;
            result.1 += child.1;
        } else {
            result.0 += 1;
            result.1 += meta.len();
        }
    }
    result
}

#[test]
#[ignore = "manual state-copy measurement; no CI time threshold"]
fn representative_state_copy_and_save_cost() {
    let source = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.provenance/state");
    for added in [0, 1000] {
        let temp = tempfile::tempdir().unwrap();
        let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
        copy_tree(&source, &layout.state_dir());
        let store = StateStore::new(layout.clone());
        let scope = store.manifest().unwrap().scopes[0].id.clone();
        let record = store
            .list_requirements(&scope)
            .unwrap()
            .into_iter()
            .find(|r| r.declared_by.is_none())
            .unwrap();
        if added != 0 {
            let path = layout.state_dir().join("copy-cost-snapshots");
            std::fs::create_dir(&path).unwrap();
            let text = "evidence".repeat(2048);
            for index in 0..added {
                std::fs::write(path.join(format!("{index}.json")), serde_json::to_vec(&json!({"schema_version":3,"record":{"id":format!("req_{index}"),"description":text}})).unwrap()).unwrap();
            }
        }
        let (files, bytes) = size(&layout.state_dir());
        let mut copy_ms = Vec::new();
        let mut save_ms = Vec::new();
        for index in 0..5 {
            let start = Instant::now();
            with_staged_state(&layout, true, |_| Ok(())).unwrap();
            copy_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            let input = serde_json::from_value(
                json!({"request_id":format!("cost_{index}"), "actor":"benchmark",
                "expected_etag":store.requirement_edit_state(&scope, &record.id).unwrap().etag,
                "update":{"scope_id":scope,"id":record.id},"relationships":null}),
            )
            .unwrap();
            let start = Instant::now();
            store.save_requirement(input).unwrap();
            save_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        copy_ms.sort_by(f64::total_cmp);
        save_ms.sort_by(f64::total_cmp);
        eprintln!("state-copy-cost added_snapshot_files={added} initial_files={files} initial_bytes={bytes} samples=5 copy_and_cleanup_ms={copy_ms:?} complete_save_ms={save_ms:?}");
    }
}
