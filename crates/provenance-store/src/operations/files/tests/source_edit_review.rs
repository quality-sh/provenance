use crate::{
    layout::ProvenanceLayout,
    operations::files::RepositoryFiles,
    publication::{
        with_repository_publication, with_staged_state_and_source_edit,
        SourceEditRecoveryFailure,
    },
    test_probes,
};
use camino::Utf8Path;

const BEFORE: &[u8] = b"before\n";
const AFTER: &[u8] = b"after\n";
const CRASH_EXIT: i32 = 86;

fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(layout.state_dir().join("value"), BEFORE).unwrap();
    std::fs::write(root.join("source.txt"), BEFORE).unwrap();
    temp
}

fn try_publish(root: &Utf8Path) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(root);
    let held = RepositoryFiles::open(root)?
        .read_bounded(Utf8Path::new("source.txt"), 1024)?;
    with_staged_state_and_source_edit(&layout, held, AFTER, |staged| {
        std::fs::write(staged.state_dir().join("value"), AFTER)?;
        Ok(())
    })
}

fn marker(layout: &ProvenanceLayout) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(layout.source_edit_marker_path()).unwrap()).unwrap()
}

fn crash(root: &Utf8Path, phase: &str) -> std::process::ExitStatus {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "operations::files::tests::source_edit_review::review_crash_child",
            "--nocapture",
        ])
        .env("PROVENANCE_REVIEW_CRASH_ROOT", root.as_str())
        .env("PROVENANCE_REVIEW_CRASH_PHASE", phase)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
}

fn recover(root: &Utf8Path) -> anyhow::Result<()> {
    with_repository_publication(&ProvenanceLayout::new(root), || Ok(()))
}

fn assert_bytes(root: &Utf8Path, source: &[u8], state: &[u8]) {
    let layout = ProvenanceLayout::new(root);
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), source);
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        state
    );
}

#[test]
fn review_crash_child() {
    let Ok(root) = std::env::var("PROVENANCE_REVIEW_CRASH_ROOT") else {
        return;
    };
    let phase = std::env::var("PROVENANCE_REVIEW_CRASH_PHASE").unwrap();
    let phase = match phase.as_str() {
        "source_edit_temp_prepared" => "source_edit_temp_prepared",
        "state_after_backup_rename" => "state_after_backup_rename",
        "state_after_install_rename" => "state_after_install_rename",
        "repository_file_after_displace" => "repository_file_after_displace",
        "repository_file_after_backup_check" => "repository_file_after_backup_check",
        "source_edit_transaction_removed" => "source_edit_transaction_removed",
        _ => panic!("unknown crash phase"),
    };
    test_probes::arm(phase, || std::process::exit(CRASH_EXIT));
    try_publish(Utf8Path::new(&root)).unwrap();
    panic!("crash phase was not reached");
}

#[test]
fn every_physical_transition_recovers_exact_file_and_state_bytes() {
    for (phase, expected) in [
        ("source_edit_temp_prepared", (BEFORE, BEFORE)),
        ("state_after_backup_rename", (AFTER, AFTER)),
        ("state_after_install_rename", (AFTER, AFTER)),
        ("repository_file_after_displace", (AFTER, AFTER)),
        ("repository_file_after_backup_check", (AFTER, AFTER)),
        ("source_edit_transaction_removed", (AFTER, AFTER)),
    ] {
        let temp = fixture();
        let root = Utf8Path::from_path(temp.path()).unwrap();
        assert_eq!(crash(root, phase).code(), Some(CRASH_EXIT), "{phase}");

        recover(root).unwrap();

        assert_bytes(root, expected.0, expected.1);
        assert!(
            !ProvenanceLayout::new(root)
                .source_edit_marker_path()
                .exists(),
            "{phase}"
        );
        let names = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            !names.iter().any(|name| name.ends_with(".tmp")),
            "{phase}: {names:?}"
        );
    }
}

#[test]
fn recovery_restores_a_displaced_target_before_finishing_forward() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    assert_eq!(
        crash(root, "repository_file_after_backup_check").code(),
        Some(CRASH_EXIT)
    );
    assert!(!root.join("source.txt").exists());

    recover(root).unwrap();

    assert_bytes(root, AFTER, AFTER);
}

#[test]
fn marker_failure_after_file_install_keeps_forward_recovery_material() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    test_probes::arm("source_edit_before_file_marker_write", || {
        anyhow::bail!("injected marker write failure")
    });

    let error = try_publish(root).unwrap_err();
    test_probes::disarm("source_edit_before_file_marker_write");

    assert!(error.to_string().contains("injected marker write failure"));
    assert_bytes(root, AFTER, AFTER);
    let layout = ProvenanceLayout::new(root);
    assert!(layout.source_edit_marker_path().exists());
    recover(root).unwrap();
    assert_bytes(root, AFTER, AFTER);
    assert!(!layout.source_edit_marker_path().exists());
}

fn crash_with_marker(root: &Utf8Path) -> (ProvenanceLayout, serde_json::Value) {
    assert_eq!(crash(root, "state_after_backup_rename").code(), Some(CRASH_EXIT));
    let layout = ProvenanceLayout::new(root);
    let value = marker(&layout);
    assert!(value["backup_leaf"].as_str().is_some());
    assert!(value["backup_identity"].is_object());
    assert!(value["backup_digest"].is_array());
    (layout, value)
}

#[test]
fn recovery_refuses_prepared_leaf_slashes_and_parent_segments() {
    for leaf in [
        ".source.txt.provenance-hop/../../outside/payload.tmp",
        "..",
    ] {
        let temp = fixture();
        let root = Utf8Path::from_path(temp.path()).unwrap();
        let (layout, mut value) = crash_with_marker(root);
        value["prepared_leaf"] = serde_json::Value::String(leaf.to_owned());
        std::fs::write(
            layout.source_edit_marker_path(),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();

        assert!(recover(root).is_err(), "{leaf}");
        assert_bytes(root, BEFORE, BEFORE);
        assert!(layout.source_edit_marker_path().exists());
    }
}

#[test]
fn recovery_refuses_backup_leaf_slashes_and_parent_segments() {
    for leaf in [
        ".source.txt.provenance-hop/../../outside/payload.backup",
        "..",
    ] {
        let temp = fixture();
        let root = Utf8Path::from_path(temp.path()).unwrap();
        let (layout, mut value) = crash_with_marker(root);
        value["backup_leaf"] = serde_json::Value::String(leaf.to_owned());
        std::fs::write(
            layout.source_edit_marker_path(),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();

        assert!(recover(root).is_err(), "{leaf}");
        assert_bytes(root, BEFORE, BEFORE);
        assert!(layout.source_edit_marker_path().exists());
    }
}

#[test]
fn same_before_bytes_with_a_different_identity_are_an_external_change() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    assert_eq!(crash(root, "state_after_backup_rename").code(), Some(CRASH_EXIT));
    let source = root.join("source.txt");
    std::fs::rename(&source, root.join("old-source.txt")).unwrap();
    std::fs::write(&source, BEFORE).unwrap();

    let error = recover(root).unwrap_err();

    assert!(matches!(
        error.downcast_ref::<SourceEditRecoveryFailure>(),
        Some(SourceEditRecoveryFailure::ExternalChange)
    ));
    assert_bytes(root, BEFORE, BEFORE);
    assert!(layout.source_edit_marker_path().exists());
}

#[cfg(unix)]
fn symlink_directory(target: &Utf8Path, link: &Utf8Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn symlink_directory(target: &Utf8Path, link: &Utf8Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

#[test]
fn recovery_refuses_a_symlinked_staged_provenance_directory() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let (layout, value) = crash_with_marker(root);
    let transaction = Utf8Path::new(value["transaction_dir"].as_str().unwrap());
    let staged_provenance = transaction.join("staged-repo/.provenance");
    std::fs::remove_dir_all(&staged_provenance).unwrap();
    let outside = root.join("outside-provenance");
    std::fs::create_dir_all(outside.join("state")).unwrap();
    std::fs::write(outside.join("state/value"), b"outside\n").unwrap();
    symlink_directory(&outside, &staged_provenance);

    assert!(recover(root).is_err());

    assert_bytes(root, BEFORE, BEFORE);
    assert_eq!(
        std::fs::read(outside.join("state/value")).unwrap(),
        b"outside\n"
    );
    assert!(layout.source_edit_marker_path().exists());
}
