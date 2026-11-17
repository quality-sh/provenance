//! Reads and sets the command of the `provenance-jsonl` merge driver in the
//! git configuration of a clone.

use camino::Utf8Path;
use provenance_macros::rule;

/// The git configuration key that holds the command of the merge driver.
pub const DRIVER_KEY: &str = "merge.provenance-jsonl.driver";

/// What init does with the merge driver of the clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverPlan {
    /// No configuration level sets the driver: init sets `command` in the
    /// local configuration.
    Set { command: String },
    /// A configuration level sets the driver, and init keeps that value.
    Present,
    /// The folder is not in a git work tree.
    NoRepository,
    /// Git did not start or did not read its configuration.
    NoGit,
    /// The project command does not run from the top of the work tree, where
    /// git runs a merge driver, so a person sets `command` by hand.
    Subdirectory { command: String },
}

/// The driver command that runs `invocation`, the command that runs
/// Provenance in the project.
pub fn driver_command(invocation: &str) -> String {
    format!("{invocation} merge-jsonl %O %A %B --output %A --path %P")
}

/// Plans the driver for the project at `path`, which `invocation` runs.
///
/// The native command comes from `PATH`, so it runs from each folder. A
/// TypeScript command names a file in the project, so it runs only from the
/// project folder.
pub fn plan(path: &Utf8Path, invocation: &str) -> DriverPlan {
    let command = driver_command(invocation);
    let at_top = match work_tree_prefix(path) {
        Probe::NoGit => return DriverPlan::NoGit,
        Probe::NoRepository => return DriverPlan::NoRepository,
        Probe::WorkTree { at_top } => at_top,
    };
    match has_driver(path) {
        None => DriverPlan::NoGit,
        Some(true) => DriverPlan::Present,
        Some(false) if at_top || invocation == crate::onboarding::NATIVE_COMMAND => {
            DriverPlan::Set { command }
        }
        Some(false) => DriverPlan::Subdirectory { command },
    }
}

/// Sets the driver that `plan` names. Returns a notice when init does not
/// set it or when the write fails.
#[rule("rule_init_sets_clone_merge_driver")]
pub fn apply(path: &Utf8Path, plan: &DriverPlan, invocation: &str) -> Option<String> {
    match plan {
        DriverPlan::Present => None,
        DriverPlan::Set { command } => {
            let failure = match run(path, &["config", "--local", DRIVER_KEY, command]) {
                Ok(output) if output.status.success() => return None,
                Ok(output) => String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                Err(error) => error.to_string(),
            };
            Some(format!(
                "Init did not set the merge driver for state files ({failure}). \
                 To set it in this clone, run: git config --local {DRIVER_KEY} '{command}'"
            ))
        }
        DriverPlan::NoRepository => Some(format!(
            "This folder is not in a git work tree, so init did not set the merge driver \
             for state files. After git init, run {invocation} init again."
        )),
        DriverPlan::NoGit => Some(format!(
            "Git did not read the configuration of this folder, so init did not set the \
             merge driver for state files. When git runs, run {invocation} init again."
        )),
        DriverPlan::Subdirectory { command } => Some(format!(
            "Git runs the merge driver for state files from the top folder of the work tree, \
             and {invocation} does not run from there, so init did not set it. To set it, \
             run: git config --local {DRIVER_KEY} '{command}' and change {invocation} to a \
             command that runs Provenance from the top folder."
        )),
    }
}

/// Whether a git configuration level sets the driver. `None` when `path` is
/// not in a work tree or git does not answer.
pub fn has_driver(path: &Utf8Path) -> Option<bool> {
    if !matches!(work_tree_prefix(path), Probe::WorkTree { .. }) {
        return None;
    }
    let output = run(path, &["config", "--get", DRIVER_KEY]).ok()?;
    match output.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

enum Probe {
    NoGit,
    NoRepository,
    /// `at_top` is true when the folder is the top of the work tree.
    WorkTree {
        at_top: bool,
    },
}

fn work_tree_prefix(path: &Utf8Path) -> Probe {
    let folder = existing_folder(path);
    let Ok(output) = run(
        path,
        &["rev-parse", "--is-inside-work-tree", "--show-prefix"],
    ) else {
        return Probe::NoGit;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    if !output.status.success() || lines.next() != Some("true") {
        return Probe::NoRepository;
    }
    // A folder that init creates is below the folder that git reads.
    let at_top = lines.next().unwrap_or_default().is_empty() && folder == path;
    Probe::WorkTree { at_top }
}

/// The nearest folder at or above `path` that exists.
fn existing_folder(path: &Utf8Path) -> &Utf8Path {
    path.ancestors()
        .find(|folder| folder.is_dir())
        .unwrap_or_else(|| Utf8Path::new("."))
}

fn run(path: &Utf8Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    provenance_store::git_process::git()
        .current_dir(existing_folder(path))
        .args(args)
        .output()
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_macros::verifies;

    fn git_ok(folder: &Utf8Path, args: &[&str]) {
        let output = run(folder, args).unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    }

    fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        git_ok(&path, &["init", "--quiet"]);
        (directory, path)
    }

    #[test]
    fn the_command_passes_the_placeholders_of_git() {
        assert_eq!(
            driver_command("provenance"),
            "provenance merge-jsonl %O %A %B --output %A --path %P"
        );
    }

    #[test]
    #[verifies("rule_init_sets_clone_merge_driver", examples)]
    fn a_folder_outside_a_work_tree_gets_a_notice() {
        let directory = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(directory.path().join("new")).unwrap();
        assert_eq!(plan(&path, "provenance"), DriverPlan::NoRepository);
        assert_eq!(has_driver(&path), None);
        let notice = apply(&path, &DriverPlan::NoRepository, "provenance").unwrap();
        assert!(notice.contains("run provenance init again"), "{notice}");
    }

    #[test]
    #[verifies("rule_init_sets_clone_merge_driver", examples)]
    fn a_local_value_stays() {
        let (_directory, path) = repository();
        git_ok(
            &path,
            &[
                "config",
                "--local",
                DRIVER_KEY,
                "/opt/bin/provenance merge-jsonl",
            ],
        );
        assert_eq!(plan(&path, "provenance"), DriverPlan::Present);
        assert_eq!(apply(&path, &DriverPlan::Present, "provenance"), None);
        let output = run(&path, &["config", "--get", DRIVER_KEY]).unwrap();
        assert_eq!(output.stdout, b"/opt/bin/provenance merge-jsonl\n");
    }

    #[test]
    #[verifies("rule_init_sets_clone_merge_driver", examples)]
    fn apply_sets_the_local_value() {
        let (_directory, path) = repository();
        let set = DriverPlan::Set {
            command: driver_command("provenance"),
        };
        assert_eq!(apply(&path, &set, "provenance"), None);
        let output = run(&path, &["config", "--local", "--get", DRIVER_KEY]).unwrap();
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "provenance merge-jsonl %O %A %B --output %A --path %P\n"
        );
        assert_eq!(has_driver(&path), Some(true));
    }

    #[test]
    #[verifies("rule_init_sets_clone_merge_driver", examples)]
    fn a_failed_write_names_the_command() {
        let directory = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let set = DriverPlan::Set {
            command: driver_command("provenance"),
        };
        let notice = apply(&path, &set, "provenance").unwrap();
        assert!(
            notice.ends_with(&format!(
                "run: git config --local {DRIVER_KEY} 'provenance merge-jsonl %O %A %B --output %A --path %P'"
            )),
            "{notice}"
        );
    }
}
