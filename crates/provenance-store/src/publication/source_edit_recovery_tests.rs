use super::{
    with_repository_publication, with_staged_state_and_source_edit, SourceEditRecoveryFailure,
};
use crate::{
    layout::ProvenanceLayout,
    operations::files::RepositoryFiles,
    test_probes,
};
use camino::Utf8Path;

const BEFORE: &[u8] = b"before\n";
const AFTER: &[u8] = b"after\n";

fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(layout.state_dir().join("value"), BEFORE).unwrap();
    std::fs::write(root.join("source.txt"), BEFORE).unwrap();
    temp
}

fn publish(root: &Utf8Path) {
    let layout = ProvenanceLayout::new(root);
    let files = RepositoryFiles::open(root).unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 1024)
        .unwrap();
    with_staged_state_and_source_edit(&layout, held, AFTER, |staged| {
        std::fs::write(staged.state_dir().join("value"), AFTER)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn crash_child() {
    let Ok(root) = std::env::var("PROVENANCE_SOURCE_EDIT_CRASH_ROOT") else {
        return;
    };
    let phase = std::env::var("PROVENANCE_SOURCE_EDIT_CRASH_PHASE").unwrap();
    let phase = match phase.as_str() {
        "source_edit_prepared" => "source_edit_prepared",
        "source_edit_backup_created" => "source_edit_backup_created",
        "source_edit_state_installed" => "source_edit_state_installed",
        "source_edit_file_installed" => "source_edit_file_installed",
        _ => panic!("unknown crash phase"),
    };
    test_probes::arm(phase, || std::process::exit(86));
    publish(Utf8Path::new(&root));
    panic!("crash phase was not reached");
}

#[test]
fn each_source_edit_phase_recovers_matching_file_and_state() {
    for phase in [
        "source_edit_prepared",
        "source_edit_backup_created",
        "source_edit_state_installed",
        "source_edit_file_installed",
    ] {
        let temp = fixture();
        let root = Utf8Path::from_path(temp.path()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "publication::source_edit_recovery_tests::crash_child",
                "--nocapture",
            ])
            .env("PROVENANCE_SOURCE_EDIT_CRASH_ROOT", root.as_str())
            .env("PROVENANCE_SOURCE_EDIT_CRASH_PHASE", phase)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "{phase}");

        let layout = ProvenanceLayout::new(root);
        with_repository_publication(&layout, || Ok(())).unwrap();
        assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), AFTER, "{phase}");
        assert_eq!(std::fs::read(layout.state_dir().join("value")).unwrap(), AFTER, "{phase}");
        assert!(!layout.source_edit_marker_path().exists(), "{phase}");
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
fn recovery_refuses_third_file_bytes_without_overwriting_them() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "publication::source_edit_recovery_tests::crash_child",
            "--nocapture",
        ])
        .env("PROVENANCE_SOURCE_EDIT_CRASH_ROOT", root.as_str())
        .env(
            "PROVENANCE_SOURCE_EDIT_CRASH_PHASE",
            "source_edit_prepared",
        )
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(86));
    std::fs::write(root.join("source.txt"), b"external\n").unwrap();
    let layout = ProvenanceLayout::new(root);

    let error = with_repository_publication(&layout, || Ok(())).unwrap_err();

    assert!(matches!(
        error.downcast_ref::<SourceEditRecoveryFailure>(),
        Some(SourceEditRecoveryFailure::ExternalChange)
    ));
    assert_eq!(
        std::fs::read(root.join("source.txt")).unwrap(),
        b"external\n"
    );
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        BEFORE
    );
    assert!(layout.source_edit_marker_path().exists());
}

#[test]
fn file_install_refusal_rolls_state_back_without_overwriting_external_bytes() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let source = root.join("source.txt");
    let changed = source.clone();
    test_probes::arm("source_edit_state_installed", move || {
        std::fs::write(&changed, b"external\n").unwrap();
        Ok(())
    });

    let result = std::panic::catch_unwind(|| publish(root));
    test_probes::disarm("source_edit_state_installed");

    assert!(result.is_err());
    let layout = ProvenanceLayout::new(root);
    assert_eq!(std::fs::read(source).unwrap(), b"external\n");
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        BEFORE
    );
    assert!(!layout.source_edit_marker_path().exists());
}

#[test]
fn final_marker_clears_when_cleanup_lost_its_result() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "publication::source_edit_recovery_tests::crash_child",
            "--nocapture",
        ])
        .env("PROVENANCE_SOURCE_EDIT_CRASH_ROOT", root.as_str())
        .env(
            "PROVENANCE_SOURCE_EDIT_CRASH_PHASE",
            "source_edit_file_installed",
        )
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(86));
    let layout = ProvenanceLayout::new(root);
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.source_edit_marker_path()).unwrap()).unwrap();
    let transaction = Utf8Path::new(marker["transaction_dir"].as_str().unwrap());
    std::fs::remove_dir_all(transaction).unwrap();

    with_repository_publication(&layout, || Ok(())).unwrap();

    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), AFTER);
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        AFTER
    );
    assert!(!layout.source_edit_marker_path().exists());
}
