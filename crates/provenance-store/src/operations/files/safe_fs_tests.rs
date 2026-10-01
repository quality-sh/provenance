use super::*;

#[test]
fn reparse_point_messages_include_the_directory_role() {
    assert_eq!(
        reparse_point_error("output parent").to_string(),
        "output parent is a reparse point"
    );
    assert_eq!(
        reparse_point_error("directory").to_string(),
        "directory is a reparse point"
    );
}

#[test]
fn directory_interface_classifies_children() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("directory")).unwrap();
    std::fs::write(temp.path().join("file"), "content").unwrap();
    let directory = Directory::open(temp.path(), "directory").unwrap();

    assert_eq!(directory.child_kind("missing").unwrap(), None);
    assert_eq!(
        directory.child_kind("directory").unwrap(),
        Some(ChildKind::Directory)
    );
    assert_eq!(directory.child_kind("file").unwrap(), Some(ChildKind::File));
}

#[test]
fn directory_rename_does_not_replace_an_existing_child() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("source"), "source").unwrap();
    std::fs::write(temp.path().join("destination"), "destination").unwrap();
    let directory = Directory::open(temp.path(), "directory").unwrap();

    let error = directory
        .rename_no_replace("source", "destination")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("destination")).unwrap(),
        "destination"
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("source")).unwrap(),
        "source"
    );
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
mod fallback {
    use super::*;

    fn unsupported(code: rustix::io::Errno) -> std::io::Error {
        std::io::Error::from_raw_os_error(code.raw_os_error())
    }

    #[test]
    fn installs_with_a_hard_link_after_unsupported_rename_errors() {
        for code in [
            rustix::io::Errno::INVAL,
            rustix::io::Errno::NOSYS,
            rustix::io::Errno::OPNOTSUPP,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("source");
            let destination = temp.path().join("destination");
            std::fs::write(&source, "source").unwrap();

            rename_no_replace_with(
                &source,
                &destination,
                || Err(unsupported(code)),
                || std::fs::hard_link(&source, &destination),
                || std::fs::remove_file(&source),
            )
            .unwrap();

            assert_eq!(std::fs::read_to_string(destination).unwrap(), "source");
            assert!(!source.exists());
        }
    }

    #[test]
    #[provenance_macros::verifies("rule_install_never_clobbers", examples)]
    fn fallback_refuses_an_existing_destination() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        std::fs::write(&source, "source").unwrap();
        std::fs::write(&destination, "destination").unwrap();

        let error = rename_no_replace_with(
            &source,
            &destination,
            || Err(unsupported(rustix::io::Errno::INVAL)),
            || std::fs::hard_link(&source, &destination),
            || std::fs::remove_file(&source),
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(source).unwrap(), "source");
        assert_eq!(
            std::fs::read_to_string(destination).unwrap(),
            "destination"
        );
    }

    #[test]
    fn fallback_reports_an_unsupported_filesystem_operation() {
        let source = Path::new("temporary");
        let destination = Path::new("manifest.json");

        let error = rename_no_replace_with(
            source,
            destination,
            || Err(unsupported(rustix::io::Errno::INVAL)),
            || Err(unsupported(rustix::io::Errno::OPNOTSUPP)),
            || panic!("unlink must not run"),
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert!(error.get_ref().unwrap().is::<NoReplaceUnsupported>());
        assert!(error.to_string().contains("link"));
        assert!(error.to_string().contains("manifest.json"));
        assert!(error.to_string().contains("does not support"));
        assert!(!error.to_string().contains("concurrent"));
    }
}
