//! The error text of a synced write that cannot make the directory of its
//! file, and the directory after a rename that fails.
//!
//! Each test is an implementation aid with no Rule, because no Rule states
//! the error text of a write to the local disk or the removal of its
//! temporary file.

use super::durability::write_synced_file;
use camino::Utf8PathBuf;

#[test]
fn a_directory_that_cannot_be_made_names_the_step_and_the_path() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = utf8(temp.path())?;
    let blocker = root.join("state");
    std::fs::write(&blocker, b"a file, not a directory")?;

    let error = write_synced_file(&blocker.join("dictionary.json"), b"{}")
        .expect_err("a file at the directory path must stop the write");

    assert_eq!(
        format!("{error:#}"),
        format!(
            "make directory {blocker}: {}",
            std::io::Error::from_raw_os_error(17)
        )
    );
    assert_eq!(std::fs::read(&blocker)?, b"a file, not a directory");
    Ok(())
}

#[test]
fn a_directory_that_cannot_be_found_names_the_step_and_the_path() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = utf8(temp.path())?;
    let blocker = root.join("state");
    std::fs::write(&blocker, b"a file, not a directory")?;
    let below = blocker.join("scopes");

    let error = write_synced_file(&below.join("dictionary.json"), b"{}")
        .expect_err("a file above the directory path must stop the write");

    assert_eq!(
        format!("{error:#}"),
        format!(
            "find directory {below}: {}",
            std::io::Error::from_raw_os_error(20)
        )
    );
    Ok(())
}

#[test]
fn a_failed_rename_leaves_no_temporary_file() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = utf8(temp.path())?;
    let target = root.join("dictionary.json");
    std::fs::create_dir_all(target.join("occupied"))?;

    let error = write_synced_file(&target, b"{}")
        .expect_err("a directory at the file path must stop the rename");

    let text = format!("{error:#}");
    assert!(text.starts_with("failed to persist temporary file: "), "{text}");
    let names = std::fs::read_dir(&root)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(names, ["dictionary.json"]);
    Ok(())
}

fn utf8(path: &std::path::Path) -> anyhow::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|path| anyhow::anyhow!("{} is not UTF-8", path.display()))
}
