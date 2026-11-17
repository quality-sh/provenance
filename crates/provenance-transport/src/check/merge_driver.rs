//! The graph finding for a clone where git sends the state files to the
//! `provenance-jsonl` merge driver and has no command for that driver.

use camino::Utf8Path;
use provenance_macros::rule;
use provenance_porcelain::check::Finding;

/// A path that the state line of `.gitattributes` matches.
const STATE_FILE: &str = ".provenance/state/check.jsonl";

const MISSING_DRIVER: &str = "Git sends .provenance/state files to the provenance-jsonl merge \
     driver, but this clone has no command for it. Run provenance init to set it.";

/// One finding when git gives the state files the `provenance-jsonl` merge
/// attribute and no configuration level sets the driver. No repository, no
/// git, or another attribute gives no finding.
#[rule("rule_check_reports_missing_merge_driver")]
pub(super) fn missing_driver_finding(repo: &Utf8Path) -> Option<Finding> {
    if !sends_state_to_driver(repo) {
        return None;
    }
    (crate::merge_driver::has_driver(repo) == Some(false)).then(|| Finding::new(MISSING_DRIVER))
}

fn sends_state_to_driver(repo: &Utf8Path) -> bool {
    provenance_store::git_process::git()
        .current_dir(repo)
        .args(["check-attr", "merge", "--", STATE_FILE])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .trim_end()
                    .ends_with(": merge: provenance-jsonl")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_macros::verifies;

    #[test]
    #[verifies("rule_check_reports_missing_merge_driver", examples)]
    fn a_folder_outside_a_repository_gives_no_finding() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join(".gitattributes"),
            ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
        )
        .unwrap();
        let repo = Utf8Path::from_path(directory.path()).unwrap();
        assert!(missing_driver_finding(repo).is_none());
    }

    #[test]
    #[verifies("rule_check_reports_missing_merge_driver", examples)]
    fn a_local_driver_gives_no_finding() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8Path::from_path(directory.path()).unwrap();
        for args in [
            &["init", "--quiet"][..],
            &[
                "config",
                "--local",
                crate::merge_driver::DRIVER_KEY,
                "provenance merge-jsonl",
            ],
        ] {
            let output = provenance_store::git_process::git()
                .current_dir(repo)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        std::fs::write(
            repo.join(".gitattributes"),
            ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
        )
        .unwrap();
        assert!(sends_state_to_driver(repo));
        assert!(missing_driver_finding(repo).is_none());
    }
}
