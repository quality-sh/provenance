use super::{restore_cargo_pair, CargoPaths, CargoRollback};
use provenance_macros::verifies;
use std::cell::RefCell;
use std::path::Path;

#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn compensation_failure_reports_both_replacement_failures() {
    let calls = RefCell::new(Vec::new());
    let error = restore_cargo_pair(
        || {
            calls.borrow_mut().push("manifest restore");
            Ok(())
        },
        || {
            calls.borrow_mut().push("lock restore");
            anyhow::bail!("lock replacement refused")
        },
        || {
            calls.borrow_mut().push("manifest compensation");
            anyhow::bail!("manifest compensation refused")
        },
        Path::new("Cargo.toml"),
        Path::new("Cargo.lock"),
    )
    .unwrap_err();

    assert_eq!(
        calls.into_inner(),
        ["manifest restore", "lock restore", "manifest compensation"]
    );
    let report = format!("{error:#}");
    assert!(report.contains("failed to restore Cargo.lock"), "{report}");
    assert!(report.contains("lock replacement refused"), "{report}");
    assert!(report.contains("failed to return Cargo.toml"), "{report}");
    assert!(report.contains("manifest compensation refused"), "{report}");
}

#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn a_concurrent_pair_edit_deterministically_refuses_all_restoration() {
    for edited_lock in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("Cargo.toml");
        let lock = directory.path().join("Cargo.lock");
        std::fs::write(&manifest, "before manifest\n").unwrap();
        let rollback = CargoRollback::capture(CargoPaths {
            manifest: manifest.clone(),
            lock: lock.clone(),
        })
        .unwrap();
        std::fs::write(&manifest, "owned manifest\n").unwrap();
        std::fs::write(&lock, "owned lock\n").unwrap();
        let rollback = rollback.observe_after().unwrap();
        if edited_lock {
            std::fs::write(&lock, "concurrent lock\n").unwrap();
        } else {
            std::fs::write(&manifest, "concurrent manifest\n").unwrap();
        }

        let error = rollback.rollback().unwrap_err();

        assert!(error.to_string().contains("neither file was restored"));
        assert_eq!(
            std::fs::read_to_string(&manifest).unwrap(),
            if edited_lock {
                "owned manifest\n"
            } else {
                "concurrent manifest\n"
            }
        );
        assert_eq!(
            std::fs::read_to_string(&lock).unwrap(),
            if edited_lock {
                "concurrent lock\n"
            } else {
                "owned lock\n"
            }
        );
    }
}
