use super::*;

#[test]
fn rollback_failure_leaves_every_ambiguous_tree_for_operator_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let output = Utf8PathBuf::from_path_buf(temp.path().join("wiki")).unwrap();
    let paths = TransactionPaths::new(&output).unwrap();
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("generation"), "old").unwrap();
    std::fs::create_dir(&paths.stage).unwrap();
    std::fs::write(paths.stage.join("generation"), "new").unwrap();
    let mut rename_count = 0;

    let error = replace_output_with(
        &output,
        &paths,
        |_, _| {
            rename_count += 1;
            if rename_count > 1 {
                Err(std::io::Error::other("injected rename failure"))
            } else {
                Ok(())
            }
        },
        |_| Ok(()),
    )
    .unwrap_err();

    assert!(matches!(error, PublishError::RollbackFailed { .. }));
    assert!(!output.exists());
    assert_eq!(
        std::fs::read_to_string(paths.backup.join("generation")).unwrap(),
        "old"
    );
    assert_eq!(
        std::fs::read_to_string(paths.stage.join("generation")).unwrap(),
        "new"
    );
}
