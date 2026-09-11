use super::journal::regular_file;
use crate::{layout::ProvenanceLayout, operations::files::FileAccessRefusal};
use camino::{Utf8Path, Utf8PathBuf};
use std::io::Read;

fn tree() -> (tempfile::TempDir, ProvenanceLayout, Utf8PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let path = root.join(".provenance/state/scopes/default/review/snapshots/one.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "safe").unwrap();
    let layout = ProvenanceLayout::new(root);
    (temp, layout, path)
}

fn link(target: &Utf8Path, path: &Utf8Path, directory: bool) {
    #[cfg(unix)]
    {
        let _ = directory;
        std::os::unix::fs::symlink(target, path).unwrap();
    }
    #[cfg(windows)]
    if directory {
        std::os::windows::fs::symlink_dir(target, path).unwrap();
    } else {
        std::os::windows::fs::symlink_file(target, path).unwrap();
    }
}

fn assert_denied(layout: &ProvenanceLayout, path: &Utf8Path) {
    let error = regular_file(layout, path).unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<FileAccessRefusal>(),
            Some(FileAccessRefusal::Denied)
        ),
        "{error:#}"
    );
}

#[test]
fn every_internal_evidence_component_rejects_links() {
    for component in [
        ".provenance",
        ".provenance/state",
        ".provenance/state/scopes",
        ".provenance/state/scopes/default",
        ".provenance/state/scopes/default/review",
        ".provenance/state/scopes/default/review/snapshots",
        ".provenance/state/scopes/default/review/snapshots/one.json",
    ] {
        let (_temp, layout, path) = tree();
        let outside = tempfile::tempdir().unwrap();
        let moved = Utf8Path::from_path(outside.path()).unwrap().join("moved");
        let selected = layout.root().join(component);
        let directory = selected.is_dir();
        std::fs::rename(&selected, &moved).unwrap();
        link(&moved, &selected, directory);
        assert_denied(&layout, &path);
    }
}

#[test]
fn internal_file_links_are_refused_even_when_the_target_is_inside() {
    let (_temp, layout, path) = tree();
    let moved = path.with_file_name("other.json");
    std::fs::rename(&path, &moved).unwrap();
    link(&moved, &path, false);
    assert_denied(&layout, &path);
}

#[test]
fn evidence_must_be_a_regular_file() {
    let (_temp, layout, path) = tree();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_denied(&layout, &path);
}

#[cfg(unix)]
#[test]
fn evidence_refuses_fifo_and_socket_without_opening_for_content() {
    let (_temp, layout, path) = tree();
    std::fs::remove_file(&path).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        path.as_std_path(),
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    assert_denied(&layout, &path);
    std::fs::remove_file(&path).unwrap();
    let socket_path = layout.root().join("socket.json");
    let _socket = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
    assert_denied(&layout, &socket_path);
}

#[test]
fn opened_evidence_keeps_its_identity_after_replacement() {
    let (_temp, layout, path) = tree();
    let mut file = regular_file(&layout, &path).unwrap();
    std::fs::rename(&path, path.with_file_name("old.json")).unwrap();
    std::fs::write(&path, "OUTSIDE_SENTINEL").unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    assert_eq!(content, "safe");
}

#[cfg(unix)]
#[test]
fn concurrent_internal_replacement_never_reads_external_evidence() {
    for directory in [false, true] {
        let (_temp, layout, path) = tree();
        let outside = tempfile::tempdir().unwrap();
        let external = Utf8Path::from_path(outside.path()).unwrap();
        std::fs::write(external.join("one.json"), "OUTSIDE_SENTINEL").unwrap();
        let selected = if directory {
            path.parent().unwrap().to_owned()
        } else {
            path.clone()
        };
        let target = if directory {
            external.to_owned()
        } else {
            external.join("one.json")
        };
        let held = selected.with_file_name("held");
        std::thread::scope(|threads| {
            threads.spawn(|| {
                for _ in 0..300 {
                    std::fs::rename(&selected, &held).unwrap();
                    link(&target, &selected, directory);
                    std::thread::yield_now();
                    std::fs::remove_file(&selected).unwrap();
                    std::fs::rename(&held, &selected).unwrap();
                }
            });
            for _ in 0..750 {
                if let Ok(mut file) = regular_file(&layout, &path) {
                    let mut content = String::new();
                    file.read_to_string(&mut content).unwrap();
                    assert_eq!(content, "safe");
                }
            }
        });
    }
}

#[cfg(windows)]
#[test]
fn evidence_refuses_an_internal_junction() {
    let (_temp, layout, path) = tree();
    let snapshots = path.parent().unwrap();
    let moved = snapshots.with_file_name("moved");
    std::fs::rename(snapshots, &moved).unwrap();
    assert!(std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J", snapshots.as_str(), moved.as_str()])
        .output()
        .unwrap()
        .status
        .success());
    assert_denied(&layout, &path);
}
