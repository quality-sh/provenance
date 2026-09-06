use std::{fs, path::PathBuf};

#[test]
fn operations_do_not_change_the_process_environment() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![root.join("operations"), root.join("operations.rs")];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let source = fs::read_to_string(&path).unwrap();
            for word in source.split(|c: char| !c.is_alphanumeric() && c != '_') {
                assert!(
                    !matches!(
                        word,
                        "set_var" | "remove_var" | "setenv" | "unsetenv" | "putenv"
                    ),
                    "{} names {word}; reads must not change the process environment",
                    path.display()
                );
            }
        }
    }
}
