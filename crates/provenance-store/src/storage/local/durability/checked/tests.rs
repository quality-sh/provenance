//! The temporary file, the permissions, the check, the removal of old
//! temporary files and the directory sync of `write_synced_file_checked`.
//! These tests have no Rule binding: they pin the store call that the
//! project file Rules of init use.
use super::write_synced_file_checked;
use camino::{Utf8Path, Utf8PathBuf};

const PREFIX: &str = ".AGENTS.md.provenance-";

fn directory() -> (tempfile::TempDir, Utf8PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dunce::canonicalize(temp.path()).unwrap()).unwrap();
    (temp, root)
}

fn names(root: &Utf8Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_temporary_file_has_the_prefix_six_letters_or_digits_and_tmp() {
    let (_temp, root) = directory();
    let path = root.join("AGENTS.md");
    let mut seen = Vec::new();

    let replaced = write_synced_file_checked(&path, b"new\n", PREFIX, None, || {
        seen = names(&root);
        Ok(())
    })
    .unwrap();

    replaced.synced.unwrap();
    let [name] = seen.as_slice() else {
        panic!("{seen:?}");
    };
    let random = name
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .unwrap_or_else(|| panic!("{name}"));
    assert_eq!(random.len(), 6, "{name}");
    assert!(random.bytes().all(|byte| byte.is_ascii_alphanumeric()), "{name}");
    assert_eq!(names(&root), ["AGENTS.md"]);
    assert_eq!(std::fs::read(&path).unwrap(), b"new\n");
}

#[cfg(unix)]
#[test]
fn the_new_file_has_the_given_permissions_or_those_of_a_created_file() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, root) = directory();
    let given = root.join("given.md");
    let plain = root.join("plain.md");
    let created = root.join("created.md");
    std::fs::File::create(&created).unwrap();
    let mode = |path: &Utf8Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o7777;

    let permissions = std::fs::Permissions::from_mode(0o640);
    let with = write_synced_file_checked(
        &given,
        b"a",
        ".given.md.provenance-",
        Some(&permissions),
        || Ok(()),
    )
    .unwrap();
    let without =
        write_synced_file_checked(&plain, b"b", ".plain.md.provenance-", None, || Ok(())).unwrap();

    assert_eq!(mode(&given), 0o640);
    assert_eq!(with.permissions.mode() & 0o7777, 0o640);
    assert_eq!(mode(&plain), mode(&created));
    assert_eq!(without.permissions.mode() & 0o7777, mode(&created));
}

#[test]
fn a_failed_check_deletes_the_temporary_file_and_keeps_the_old_bytes() {
    let (_temp, root) = directory();
    let path = root.join("AGENTS.md");
    std::fs::write(&path, "old\n").unwrap();

    let error = write_synced_file_checked(&path, b"new\n", PREFIX, None, || {
        anyhow::bail!("the file changed")
    })
    .unwrap_err();

    assert_eq!(format!("{error:#}"), "the file changed");
    assert_eq!(std::fs::read(&path).unwrap(), b"old\n");
    assert_eq!(names(&root), ["AGENTS.md"]);
}

/// The removal takes each regular file with exactly the name form, and
/// keeps a name near the form, a directory and a symbolic link.
#[test]
fn the_removal_of_old_temporary_files_takes_only_regular_files_of_the_name_form() {
    let (_temp, root) = directory();
    let path = root.join("AGENTS.md");
    let exact = [".AGENTS.md.provenance-abc123.tmp", ".AGENTS.md.provenance-ZZZZZZ.tmp"];
    let near = [
        ".AGENTS.md.provenance-abc12.tmp",
        ".AGENTS.md.provenance-abc1234.tmp",
        ".AGENTS.md.provenance-abc-12.tmp",
        ".AGENTS.md.provenance-abc123.tmp.old",
        ".AGENTS.md.provenance-4242-0.tmp",
        ".README.md.provenance-abc123.tmp",
        "AGENTS.md.provenance-abc123.tmp",
    ];
    for name in exact.iter().chain(&near) {
        std::fs::write(root.join(name), "kept?\n").unwrap();
    }
    let folder = ".AGENTS.md.provenance-folder.tmp";
    std::fs::create_dir(root.join(folder)).unwrap();
    #[cfg(unix)]
    let link = ".AGENTS.md.provenance-link00.tmp";
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join(near[0]), root.join(link)).unwrap();

    let replaced = write_synced_file_checked(&path, b"new\n", PREFIX, None, || Ok(())).unwrap();

    replaced.synced.unwrap();
    let mut expected: Vec<String> = near.iter().map(|name| (*name).to_owned()).collect();
    expected.push(folder.to_owned());
    #[cfg(unix)]
    expected.push(link.to_owned());
    expected.push("AGENTS.md".to_owned());
    expected.sort();
    assert_eq!(names(&root), expected);
}

/// A directory sync that fails after the rename gives the new file with its
/// permissions and the failure of the sync, a `ChangedUnsynced`.
#[cfg(unix)]
#[test]
fn a_failed_directory_sync_after_the_rename_gives_the_new_file_and_the_failure() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, root) = directory();
    let folder = root.join("project");
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("AGENTS.md");
    std::fs::write(&path, "old\n").unwrap();
    let permissions = std::fs::Permissions::from_mode(0o640);

    let replaced = write_synced_file_checked(&path, b"new\n", PREFIX, Some(&permissions), || {
        let closed = std::fs::Permissions::from_mode(0o300);
        std::fs::set_permissions(&folder, closed)?;
        Ok(())
    });
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();

    let replaced = replaced.unwrap();
    assert_eq!(replaced.permissions.mode() & 0o7777, 0o640);
    let failure = replaced.synced.unwrap_err();
    assert_eq!(
        format!("{:#}", anyhow::Error::new(failure)),
        format!("sync {folder}: {}", std::io::Error::from_raw_os_error(13))
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"new\n");
    assert_eq!(names(&folder), ["AGENTS.md"]);
}

/// On Windows, the rename replaces a read-only file, and the new file is
/// read-only when the caller gives read-only permissions.
#[cfg(windows)]
#[test]
fn a_read_only_file_is_replaced_and_the_new_file_is_read_only() {
    let (_temp, root) = directory();
    let path = root.join("AGENTS.md");
    std::fs::write(&path, "old\n").unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions.clone()).unwrap();

    let replaced =
        write_synced_file_checked(&path, b"new\n", PREFIX, Some(&permissions), || Ok(())).unwrap();

    replaced.synced.unwrap();
    assert!(replaced.permissions.readonly());
    assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
    assert_eq!(std::fs::read(&path).unwrap(), b"new\n");
    assert_eq!(names(&root), ["AGENTS.md"]);
    let mut writable = std::fs::metadata(&path).unwrap().permissions();
    writable.set_readonly(false);
    std::fs::set_permissions(&path, writable).unwrap();
}
