//! The writes of the index of record changes: a read cache that the process
//! cannot write, the freshness policies that write no revision, the text
//! of a refusal while versions wait, and two processes that write the index
//! at the same time.

use super::{
    fixture, history, operation, records, repository, statement_edits, Counted, MemoryFileVersions,
    ONE_READER,
};
use camino::Utf8Path;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::protocol::{StampPolicy, Stamped};
use provenance_core::review::{ReviewHistoryPage, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};
use provenance_macros::verifies;
use provenance_store::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_store::repository::Repository;
use provenance_store::review::read_history;
use serde_json::{json, Value};
use std::sync::Arc;

/// The history of `req_a` under `policy`, with its stamp.
async fn stamped(
    repository: &Repository,
    policy: FreshnessPolicy,
) -> anyhow::Result<Stamped<ReviewHistoryPage>> {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: StableId::new("req_a").unwrap(),
        limit: 50,
        cursor: None,
    };
    read_history(repository, ReadPolicy::with_freshness(policy), query).await
}

/// The entry ids of a history page.
fn ids(page: &ReviewHistoryPage) -> Vec<&str> {
    page.entries.iter().map(|entry| entry.id.as_str()).collect()
}

/// Makes the read cache of the repository at `root` with a read that
/// indexes nothing, and gives the stamp of that read.
async fn made_cache(root: &Utf8Path) -> provenance_core::protocol::Stamp {
    let empty = repository(root).with_file_versions(Arc::new(MemoryFileVersions::new(&[])));
    stamped(&empty, FreshnessPolicy::CatchUp)
        .await
        .unwrap()
        .stamp
}

/// A read under `annotate_only` or `refuse_stale` writes the index rows
/// that it made, and writes no projection revision: the stamp of each
/// later read is at the same serial, digest and instance.
#[tokio::test]
#[verifies("rule_refuse_stale_writes_no_revision", examples)]
async fn a_read_under_each_policy_writes_index_rows_and_no_revision() {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let made = made_cache(root).await;
    let commits = statement_edits(&temp, 3);
    let mut calls = Vec::new();
    for policy in [
        FreshnessPolicy::AnnotateOnly,
        FreshnessPolicy::RefuseStale,
        FreshnessPolicy::CatchUp,
    ] {
        let versions = Counted::new_lists(&commits);
        let opened = repository(root).with_file_versions(versions.clone());
        let answer = stamped(&opened, policy).await.unwrap();
        assert_eq!(ids(&answer.result), ["c0", "c1", "c2", "working"]);
        assert_eq!(
            (
                answer.stamp.serial,
                &answer.stamp.digest,
                &answer.stamp.instance_id
            ),
            (made.serial, &made.digest, &made.instance_id),
            "{policy:?}"
        );
        assert_eq!(answer.stamp.attested, ["record_history"]);
        calls.push(versions.calls());
    }
    assert_eq!(calls, [6, 0, 0], "the first read wrote the index");
}

/// A read whose first page fits answers past a failed write of the index,
/// as it answers when it needs no write: a read-only checkout answers at the
/// serial that its cache holds.
#[cfg(unix)]
#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_history_read_answers_when_the_read_cache_cannot_be_written() {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let made = made_cache(root).await;
    let Some(_restore) = read_only::cache(root) else {
        return;
    };
    let versions = Counted::new_lists(&statement_edits(&temp, 3));
    let opened = repository(root).with_file_versions(versions.clone());
    for (policy, word) in [
        (FreshnessPolicy::AnnotateOnly, StampPolicy::AnnotateOnly),
        (FreshnessPolicy::CatchUp, StampPolicy::CatchUpFailed),
    ] {
        let answer = stamped(&opened, policy).await.unwrap();
        assert_eq!(ids(&answer.result), ["c0", "c1", "c2", "working"]);
        assert_eq!(answer.stamp.serial, made.serial);
        assert_eq!(answer.stamp.policy, word);
        assert_eq!(versions.calls(), 6, "the index stays empty");
    }
}

/// Past 1,000 versions to index, a read that cannot write the index gives
/// the page budget refusal on each call, and its cause says that the index
/// could not be written, with the path relative to the repository.
#[cfg(unix)]
#[tokio::test]
#[verifies("rule_history_reads_bound_file_versions", examples)]
async fn a_refusal_says_that_the_index_could_not_be_written() {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    made_cache(root).await;
    let Some(_restore) = read_only::cache(root) else {
        return;
    };
    let versions = Counted::new_lists(&statement_edits(&temp, 1_001));
    let opened = repository(root).with_file_versions(versions.clone());
    let (path, _) = records(&temp);
    let start = format!(
        "versions of {path} that are not in the index of record changes: 1001; the history read \
         could not write the index, so a repeat does not continue: "
    );
    for _ in 0..2 {
        let error = history(&opened, "req_a").await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<ReadFailure>(),
            Some(&ReadFailure::PageBudgetExceeded)
        );
        let text = error.to_string();
        eprintln!("refusal: {error:#}");
        assert!(text.starts_with(&start), "{text}");
        assert!(text.len() > start.len(), "the cause of the write: {text}");
        assert!(!text.contains(root.as_str()), "{text}");
        assert_eq!(versions.calls(), 2_000, "no read continues");
    }
    let request = json!({"record_kind": "requirement", "record_id": "req_a"});
    let refused = operation(&opened, "review-history", request).await;
    assert_eq!(refused, Err(json!({"kind": "page_budget_exceeded"})));
}

/// A read that refuses while more versions wait says how many versions it
/// put in the index and how many wait, and that a repeat continues.
#[tokio::test]
#[verifies("rule_history_reads_bound_file_versions", examples)]
async fn a_refusal_while_versions_wait_says_that_a_repeat_continues() {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let versions = Counted::new_lists(&statement_edits(&temp, 1_500));
    let opened = repository(root).with_file_versions(versions.clone());
    let (path, _) = records(&temp);
    let error = history(&opened, "req_a").await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded)
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the history read put 1000 versions of {path} in the index of record changes; \
             versions that wait: 500; a repeat of the read continues"
        )
    );
    assert_eq!(history(&opened, "req_a").await.unwrap().entries.len(), 50);
}

/// The variable that makes a run of the test binary one of the two writer
/// processes: it names the repository root.
const WRITER_ROOT: &str = "PROVENANCE_HISTORY_INDEX_WRITER_ROOT";

/// The number of versions that each writer process indexes.
const WRITER_VERSIONS: usize = 600;

/// Two processes that read the same record file at the same time both
/// answer, both write the same index rows, and a later read of the file
/// reads no text and gives the same answer.
#[tokio::test]
async fn two_processes_write_the_index_at_the_same_time() {
    if let Ok(root) = std::env::var(WRITER_ROOT) {
        let versions = Counted::new_lists(&statement_edits(&root, WRITER_VERSIONS));
        let opened = repository(Utf8Path::new(&root)).with_file_versions(versions);
        let page = history(&opened, "req_a").await.unwrap();
        println!("answer: {}", serde_json::to_string(&page).unwrap());
        return;
    }
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    made_cache(root).await;
    let writer = || {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "index_writes::two_processes_write_the_index_at_the_same_time",
                "--exact",
                "--nocapture",
            ])
            .env(WRITER_ROOT, root.as_str())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let writers = [writer(), writer()];
    let mut answers = Vec::new();
    for writer in writers {
        let output = writer.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            output.status.success(),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answer = stdout
            .lines()
            .find_map(|line| line.strip_prefix("answer: "))
            .unwrap_or_else(|| panic!("no answer in {stdout}"));
        answers.push(serde_json::from_str::<Value>(answer).unwrap());
    }
    assert_eq!(answers[0], answers[1]);
    let versions = Counted::new_lists(&statement_edits(&temp, WRITER_VERSIONS));
    let opened = repository(root).with_file_versions(versions.clone());
    let page = history(&opened, "req_a").await.unwrap();
    assert_eq!(versions.calls(), 0, "the writers indexed the file");
    assert_eq!(serde_json::to_value(page).unwrap(), answers[0]);
}

#[cfg(unix)]
mod read_only {
    use camino::Utf8Path;
    use provenance_store::layout::ProvenanceLayout;
    use std::os::unix::fs::PermissionsExt;

    /// Gives the cache directory back its mode.
    pub struct Restore(camino::Utf8PathBuf, std::fs::Permissions);

    impl Drop for Restore {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }

    /// Makes the cache directory below `root` one that the process cannot
    /// write. `None` when this user writes through the mode, so the test
    /// cannot make the directory unwritable.
    pub fn cache(root: &Utf8Path) -> Option<Restore> {
        let directory = ProvenanceLayout::new(root).cache_dir();
        let old = std::fs::metadata(&directory).unwrap().permissions();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
        let restore = Restore(directory.clone(), old);
        let probe = directory.join("probe");
        if std::fs::write(&probe, b"").is_ok() {
            std::fs::remove_file(probe).unwrap();
            eprintln!("SKIPPED: this user writes through mode 0o555");
            return None;
        }
        Some(restore)
    }
}
