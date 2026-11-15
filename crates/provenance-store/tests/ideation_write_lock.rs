//! The lock of the six ideation writes of the web routes: create-proposal,
//! create-proposal-assertion, create-contribution, update-contribution,
//! create-synthesis-packet and update-synthesis-packet.
//!
//! Each case starts from the state of the memory fixture with a
//! contribution, a synthesis packet that suggests `proposal_one`, and
//! `proposal_one` with no assertion. The first write asserts the proposal;
//! the other five add and change records that no assertion cites.

use crate::memory;

use camino::Utf8Path;
use memory::{id, key, scope, shard};
use provenance_core::Contribution;
use provenance_store::repository::Repository;
use provenance_store::state_store::StateStore;
use provenance_store::storage::{StateKey, StateView};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn target() -> Value {
    json!({"artifact_type":"requirement","artifact_id":"req_expiry"})
}

fn contribution(id: &str, claims: &Value, evidence: &Value) -> Value {
    json!({
        "scope_id":"default", "id":id, "target":target(), "participant_slot":"reviewer",
        "stance":"support", "strongest_finding":"Observed", "evidence_references":evidence,
        "material_claims":claims, "risks":[], "objections":[], "challenges":[],
        "suggested_artifact_changes":[], "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct"}, "open_questions":[]
    })
}

fn packet(id: &str, suggested: &Value) -> Value {
    json!({
        "scope_id":"default", "id":id, "target":target(), "summary":"Adjudicated",
        "consensus":[], "contested_claims":[], "minority_objections":[], "evidence_gaps":[],
        "unsupported_speculation":[], "open_questions":[], "suggested_artifacts":suggested,
        "required_human_decisions":[]
    })
}

fn proposal(id: &str, claims: &Value) -> Value {
    json!({
        "scope_id":"default", "id":id, "proposal_key":id,
        "proposal_type":"requirement_candidate", "title":"Candidate",
        "summary":"Clarify the requirement.",
        "traceability":{"target":target(), "source_ids":[], "evidence_references":[],
            "supporting_claim_ids":claims},
        "builds_on":[], "promotion_state":"proposed"
    })
}

/// The initial state of the memory fixture and the evidence of `proposal_one`.
fn entries() -> BTreeMap<StateKey, Vec<u8>> {
    let mut entries = memory::initial_state().into_entries();
    let claim = json!([{"claim_id":"claim_a","statement":"Observed","evidence_type":"source",
        "evidence_reference_ids":["evidence_a"]}]);
    let evidence = json!([{"reference_id":"evidence_a","evidence_type":"source",
        "summary":"Pinned"}]);
    let suggested = json!([{"proposal_id":"proposal_one","proposal_key":"proposal_one",
        "proposal_type":"requirement_candidate","summary":"Candidate",
        "origin_participant_slots":["reviewer"]}]);
    for (file, row) in [
        (
            "contributions",
            contribution("contribution_evidence", &claim, &evidence),
        ),
        (
            "synthesis_packets",
            packet("synthesis_evidence", &suggested),
        ),
        (
            "proposal_cards",
            proposal("proposal_one", &json!(["claim_a"])),
        ),
    ] {
        let path = format!("scopes/default/ideation/{file}.jsonl");
        entries.insert(key(&path), shard(&[row]));
    }
    entries
}

fn input<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

/// Asserts `proposal_one`. Each other write refuses the state before it.
fn assert_proposal_one(store: &StateStore) {
    store
        .assert_proposal(input(json!({
            "scope_id":"default", "id":"assertion_one", "proposal_id":"proposal_one",
            "synthesis_packet_id":"synthesis_evidence", "supporting_claim_ids":["claim_a"]
        })))
        .unwrap();
}

/// Runs the six writes in the order of the module comment.
fn six_writes(store: &StateStore) {
    assert_proposal_one(store);
    store
        .create_contribution(input(contribution(
            "contribution_two",
            &json!([]),
            &json!([]),
        )))
        .unwrap();
    store
        .update_contribution(input(json!({
            "scope_id":"default", "id":"contribution_two", "strongest_finding":"Revised"
        })))
        .unwrap();
    store
        .create_synthesis_packet(input(packet("synthesis_two", &json!([]))))
        .unwrap();
    store
        .update_synthesis_packet(input(json!({
            "scope_id":"default", "id":"synthesis_two", "summary":"Revised"
        })))
        .unwrap();
    store
        .create_proposal_card(input(proposal("proposal_two", &json!([]))))
        .unwrap();
}

/// The six writes are in `store`.
fn assert_landed(store: &StateStore) {
    let assertions = store.list_assertion_records(&scope()).unwrap();
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].proposal_id, id("proposal_one"));
    let contributions = store.list_contributions(&scope()).unwrap();
    let two = contributions
        .iter()
        .find(|record| record.id == id("contribution_two"))
        .unwrap();
    assert_eq!(two.strongest_finding, "Revised");
    let packets = store.list_synthesis_packets(&scope()).unwrap();
    let two = packets
        .iter()
        .find(|record| record.id == id("synthesis_two"))
        .unwrap();
    assert_eq!(two.summary, "Revised");
    let proposals = store.list_proposal_definitions(&scope()).unwrap();
    assert!(proposals
        .iter()
        .any(|record| record.id == id("proposal_two")));
}

/// A folder repository at `root` whose state holds [`entries`].
fn folder_repository(root: &std::path::Path) -> Repository {
    let state = root.join(".provenance/state");
    for (key, bytes) in entries() {
        let path = state.join(key.as_str());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    Repository::open(root, "default").unwrap()
}

/// The files below `dir`, as paths relative to it with `/` separators.
fn files_below(dir: &Utf8Path) -> Vec<String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let relative = path.strip_prefix(base).unwrap();
                let parts: Vec<_> = relative.iter().map(|p| p.to_string_lossy()).collect();
                out.push(parts.join("/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir.as_std_path(), dir.as_std_path(), &mut out);
    out.sort();
    out
}

/// Characterization: the files that the six writes leave in the locks
/// directory and the ideation directory of a folder repository. No Rule
/// names these files.
#[test]
fn six_writes_on_a_folder_leave_these_files() {
    let root = tempfile::tempdir().unwrap();
    let repository = folder_repository(root.path());
    let layout = repository.layout().clone();
    let locks = layout.cache_dir().join("locks");
    let before = files_below(&locks);

    six_writes(&repository.store());

    assert_landed(&repository.store());
    let added: Vec<_> = files_below(&locks)
        .into_iter()
        .filter(|file| !before.contains(file))
        .collect();
    assert_eq!(
        added,
        [
            "scopes/default/ideation/assertions.jsonl.lock",
            "scopes/default/ideation/contributions.jsonl.lock",
            "scopes/default/ideation/proposal_cards.jsonl.lock",
            "scopes/default/ideation/synthesis_packets.jsonl.lock",
        ],
        "{before:?}"
    );
    let ideation = layout.state_dir().join("scopes/default/ideation");
    assert_eq!(
        files_below(&ideation),
        [
            "assertions.jsonl",
            "contributions.jsonl",
            "proposal_cards.jsonl",
            "synthesis_packets.jsonl"
        ]
    );
}

fn contribution_input(id: &str) -> provenance_store::state_store::CreateContributionInput {
    input(contribution(id, &json!([]), &json!([])))
}

/// Characterization: two writers on two handles of one folder repository
/// run at the same time, and each write lands. No Rule names the count.
/// The interleaving depends on the scheduler: the gap between the read and
/// the write of one writer is inside the store and has no public seam.
#[test]
fn two_writers_on_one_folder_lose_no_write() {
    let root = tempfile::tempdir().unwrap();
    let first = folder_repository(root.path());
    assert_proposal_one(&first.store());
    let second = Repository::open(root.path(), "default").unwrap();
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));
    let writers: Vec<_> = [("a", first), ("b", second)]
        .into_iter()
        .map(|(name, repository)| {
            let start = std::sync::Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                for index in 0..5 {
                    let id = format!("contribution_{name}_{index}");
                    repository
                        .store()
                        .create_contribution(contribution_input(&id))
                        .unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let repository = Repository::open(root.path(), "default").unwrap();
    let contributions: Vec<Contribution> = repository.store().list_contributions(&scope()).unwrap();
    assert_eq!(contributions.len(), 11);
}

/// Whether a thread of this process waits for a `flock` on the file at
/// `path`. The kernel lists each waiting request in `/proc/locks` with `->`
/// and the device and inode of the file.
#[cfg(target_os = "linux")]
fn a_thread_waits_on(path: &Utf8Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let inode = format!(":{}", std::fs::metadata(path).unwrap().ino());
    let pid = std::process::id().to_string();
    std::fs::read_to_string("/proc/locks")
        .unwrap()
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .any(|fields| {
            fields.get(1) == Some(&"->")
                && fields.get(2) == Some(&"FLOCK")
                && fields.get(5) == Some(&pid.as_str())
                && fields.get(6).is_some_and(|device| device.ends_with(&inode))
        })
}

/// Characterization: an ideation write waits while another holder, such as
/// another process, holds the publication lock file of a folder
/// repository. No Rule names the lock file.
///
/// The test reads the wait from the kernel's list of waiting lock requests,
/// which only Linux gives.
#[test]
#[cfg(target_os = "linux")]
fn an_ideation_write_waits_for_the_publication_lock_file() {
    use fs2::FileExt;
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap();
    let repository = folder_repository(root.path());
    assert_proposal_one(&repository.store());
    let lock_path: camino::Utf8PathBuf = repository.layout().publication_lock_path();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    lock.lock_exclusive().unwrap();

    let writer = std::thread::spawn({
        let repository = repository.clone();
        move || {
            repository
                .store()
                .create_contribution(contribution_input("contribution_waited"))
                .unwrap();
        }
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    while !a_thread_waits_on(&lock_path) {
        assert!(!writer.is_finished(), "the write ran under the held lock");
        assert!(
            Instant::now() < deadline,
            "the write did not wait on the publication lock file"
        );
        std::thread::yield_now();
    }
    assert!(!writer.is_finished(), "the write ran under the held lock");
    FileExt::unlock(&lock).unwrap();
    writer.join().unwrap();

    let contributions = repository.store().list_contributions(&scope()).unwrap();
    assert!(contributions
        .iter()
        .any(|record| record.id == id("contribution_waited")));
}

/// Takes the write permission from a directory, and gives it back on drop.
#[cfg(unix)]
struct ReadOnly(std::path::PathBuf);

#[cfg(unix)]
impl Drop for ReadOnly {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// Takes the write permission from `dir` until the returned guard drops.
#[cfg(unix)]
fn read_only(dir: &std::path::Path) -> ReadOnly {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let guard = ReadOnly(dir.to_owned());
    assert!(
        std::fs::create_dir(dir.join("probe")).is_err(),
        "the test user can write a read-only directory"
    );
    guard
}

/// On each platform the six writes over memory land and make no file in the
/// working-tree directory. On Unix the directory is also read-only, so a
/// write that tried to make a file there would fail.
#[test]
#[provenance_macros::verifies("rule_engine_runs_on_any_storage", examples)]
fn six_writes_over_memory_make_no_file() {
    let root = tempfile::tempdir().unwrap();
    let recorded = std::sync::Arc::new(memory::Recorded::default());
    let (_storage, repository) =
        memory::memory_repository_over(root.path(), StateView::new(entries()), recorded);
    #[cfg(unix)]
    let _guard = read_only(root.path());

    six_writes(&repository.store());

    assert_landed(&repository.view().unwrap());
    let entries: Vec<_> = std::fs::read_dir(root.path()).unwrap().collect();
    assert!(
        entries.is_empty(),
        "the repository wrote files: {entries:?}"
    );
}
