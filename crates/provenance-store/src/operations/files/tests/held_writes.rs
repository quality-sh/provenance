use super::super::*;
use camino::Utf8Path;
use sha2::Digest as _;

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf, RepositoryFiles) {
    let temporary = tempfile::tempdir().unwrap();
    let path =
        camino::Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let files = RepositoryFiles::open(&path).unwrap();
    (temporary, path, files)
}

#[test]
fn bounded_read_and_replace_preserve_exact_utf8_bytes() {
    let (_temporary, path, files) = repository();
    let original = "first\r\nGrüße\r\n".as_bytes();
    let replacement = "second\r\nκόσμος\r\n".as_bytes();
    std::fs::write(path.join("source.txt"), original).unwrap();

    let held = files
        .read_bounded(Utf8Path::new("source.txt"), original.len())
        .unwrap();
    assert_eq!(held.bytes(), original);
    let expected_digest: [u8; 32] = sha2::Sha256::digest(original).into();
    assert_eq!(held.digest(), expected_digest);

    let prepared = held.create_temp(replacement).unwrap();
    held.compare_and_swap(prepared).unwrap();
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), replacement);
}

#[test]
fn bounded_read_refuses_a_file_above_the_limit() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"12345").unwrap();

    assert!(matches!(
        files.read_bounded(Utf8Path::new("source.txt"), 4),
        Err(RepositoryFileRefusal::TooLarge { limit: 4 })
    ));
}

#[test]
fn bounded_read_refuses_invalid_utf8() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), [0xff, 0xfe]).unwrap();

    assert!(matches!(
        files.read_bounded(Utf8Path::new("source.txt"), 2),
        Err(RepositoryFileRefusal::InvalidUtf8)
    ));
}

#[test]
fn compare_and_swap_refuses_changed_bytes_and_preserves_them() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    std::fs::write(path.join("source.txt"), b"third party").unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed)
    ));
    assert_eq!(
        std::fs::read(path.join("source.txt")).unwrap(),
        b"third party"
    );
}

#[test]
fn compare_and_swap_refuses_a_new_identity_with_the_same_bytes() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"same bytes").unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    std::fs::rename(path.join("source.txt"), path.join("old.txt")).unwrap();
    std::fs::write(path.join("source.txt"), b"same bytes").unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed)
    ));
    assert_eq!(
        std::fs::read(path.join("source.txt")).unwrap(),
        b"same bytes"
    );
}

#[cfg(unix)]
#[test]
fn held_write_refuses_a_symlink_leaf_and_parent() {
    use std::os::unix::fs::symlink;
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("actual.txt"), b"actual").unwrap();
    symlink("actual.txt", path.join("leaf.txt")).unwrap();
    std::fs::create_dir(path.join("actual-parent")).unwrap();
    std::fs::write(path.join("actual-parent/source.txt"), b"actual").unwrap();
    symlink("actual-parent", path.join("linked-parent")).unwrap();

    for selected in ["leaf.txt", "linked-parent/source.txt"] {
        assert!(matches!(
            files.read_bounded(Utf8Path::new(selected), 100),
            Err(RepositoryFileRefusal::Denied)
        ));
    }
}

#[cfg(windows)]
#[test]
fn held_write_refuses_a_reparse_leaf_and_parent() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("actual.txt"), b"actual").unwrap();
    symlink_file(path.join("actual.txt"), path.join("leaf.txt")).unwrap();
    std::fs::create_dir(path.join("actual-parent")).unwrap();
    std::fs::write(path.join("actual-parent/source.txt"), b"actual").unwrap();
    symlink_dir(path.join("actual-parent"), path.join("linked-parent")).unwrap();

    for selected in ["leaf.txt", "linked-parent/source.txt"] {
        assert!(matches!(
            files.read_bounded(Utf8Path::new(selected), 100),
            Err(RepositoryFileRefusal::Denied)
        ));
    }
}

#[test]
fn held_write_refuses_paths_outside_the_root() {
    let (_temporary, _path, files) = repository();
    for selected in ["../outside.txt", "/outside.txt"] {
        assert!(matches!(
            files.read_bounded(Utf8Path::new(selected), 100),
            Err(RepositoryFileRefusal::Denied)
        ));
    }
}

#[cfg(unix)]
#[test]
fn compare_and_swap_keeps_the_original_permissions() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    std::fs::set_permissions(
        path.join("source.txt"),
        std::fs::Permissions::from_mode(0o754),
    )
    .unwrap();
    let before = std::fs::metadata(path.join("source.txt")).unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let identity = held.identity().clone();
    let prepared = held.create_temp(b"replacement").unwrap();

    held.compare_and_swap(prepared).unwrap();

    let after = std::fs::metadata(path.join("source.txt")).unwrap();
    assert_eq!(after.mode() & 0o7777, before.mode() & 0o7777);
    assert_eq!(after.uid(), before.uid());
    assert_eq!(after.gid(), before.gid());
    assert_ne!(
        files
            .read_bounded(Utf8Path::new("source.txt"), 100)
            .unwrap()
            .identity(),
        &identity
    );
}

#[cfg(windows)]
#[test]
fn compare_and_swap_keeps_the_read_only_permission() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let mut permissions = std::fs::metadata(path.join("source.txt"))
        .unwrap()
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path.join("source.txt"), permissions).unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();

    held.compare_and_swap(prepared).unwrap();

    assert!(std::fs::metadata(path.join("source.txt"))
        .unwrap()
        .permissions()
        .readonly());
    assert_eq!(
        std::fs::read(path.join("source.txt")).unwrap(),
        b"replacement"
    );
}

#[cfg(windows)]
#[test]
fn compare_and_swap_copies_a_null_dacl() {
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL};
    use windows_sys::Win32::Security::Authorization::{
        GetSecurityInfo, SetSecurityInfo, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
    use windows_sys::Win32::System::Memory::LocalFree;

    const READ_CONTROL: u32 = 0x0002_0000;
    const WRITE_DAC: u32 = 0x0004_0000;
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let target = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC)
        .open(path.join("source.txt"))
        .unwrap();
    assert_eq!(
        unsafe {
            SetSecurityInfo(
                target.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        },
        ERROR_SUCCESS
    );
    drop(target);
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();

    held.compare_and_swap(prepared).unwrap();

    let installed = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL)
        .open(path.join("source.txt"))
        .unwrap();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    assert_eq!(
        unsafe {
            GetSecurityInfo(
                installed.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        },
        ERROR_SUCCESS
    );
    assert!(dacl.is_null());
    unsafe { LocalFree(descriptor as HLOCAL) };
}
