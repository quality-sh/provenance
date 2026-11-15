//! The merge of the state files of swarm runs for each history of a small
//! domain, against a record rule that this test states by itself.
//!
//! The base lands contribution `c`. Each side lands up to two runs of a pool:
//! `c` with other content, `c` with the base content, contribution `d`, and a
//! run that asserts proposal `p` on the claim of `c`. A history that a landing
//! refuses cannot exist, so the test leaves it out.
//!
//! The rule: a record that the two sides hold equal keeps that value. A record
//! that only one side changes from the base gets the value of that side. A
//! record that the two sides change differently is a conflict, and the result
//! keeps our value. Each result passes the check of the store. A clean merge
//! gives the same files in the two directions. The merges of a merge result
//! with a third branch, and of two merge results after one more run on each,
//! follow the same rule.

use crate::memory::{key, memory_repository_over, scope, Recorded};
use provenance_macros::verifies;
use provenance_store::merge::{merge_records, MergeOutcome};
use provenance_store::repository::Repository;
use provenance_store::state_store::IdeationLandingBatch;
use provenance_store::storage::memory::MemoryStateStorage;
use provenance_store::storage::{StateKey, StateStorage, StateView};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

type Entries = BTreeMap<StateKey, Vec<u8>>;

/// The record of each file and id that a read of the store gives.
type Read = BTreeMap<(&'static str, String), Value>;

const FILES: [&str; 5] = [
    "contributions",
    "synthesis_packets",
    "proposal_cards",
    "assertions",
    "dispositions",
];

fn file_key(file: &str) -> StateKey {
    key(&format!("scopes/default/ideation/{file}.jsonl"))
}

fn contribution(id: &str, finding: &str) -> Value {
    json!({
        "scope_id": "default", "id": id,
        "target": {"artifact_type": "requirement", "artifact_id": "req_expiry"},
        "participant_slot": "slot_a", "stance": "support", "strongest_finding": finding,
        "evidence_references": [{"reference_id": format!("evidence_{id}"),
            "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": format!("claim_{id}"), "statement": finding,
            "evidence_type": "source", "evidence_reference_ids": [format!("evidence_{id}")]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct."}, "open_questions": []
    })
}

/// A run that suggests, lands and asserts proposal `p` on the claim of `c`.
fn assertion_run() -> Value {
    let target = json!({"artifact_type": "requirement", "artifact_id": "req_expiry"});
    json!({
        "synthesis_packets": [{"scope_id": "default", "id": "packet_p", "target": target,
            "summary": "Adjudicated", "consensus": [], "contested_claims": [],
            "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
            "open_questions": [],
            "suggested_artifacts": [{"proposal_id": "p", "proposal_key": "p",
                "proposal_type": "requirement_candidate", "summary": "Candidate",
                "origin_participant_slots": ["slot_a"]}],
            "required_human_decisions": []}],
        "proposals": [{"scope_id": "default", "id": "p", "proposal_key": "p",
            "proposal_type": "requirement_candidate", "title": "P", "summary": "Candidate",
            "traceability": {"target": target, "source_ids": [], "evidence_references": [],
                "supporting_claim_ids": ["claim_c"]},
            "promotion_state": "proposed"}],
        "assertions": [{"scope_id": "default", "id": "a", "proposal_id": "p",
            "synthesis_packet_id": "packet_p", "supporting_claim_ids": ["claim_c"]}]
    })
}

fn pool() -> Vec<Value> {
    vec![
        json!({"contributions": [contribution("c", "Other.")]}),
        json!({"contributions": [contribution("c", "Base.")]}),
        json!({"contributions": [contribution("d", "D.")]}),
        assertion_run(),
    ]
}

/// The memory repositories that land the runs and read the results.
struct Domain {
    root: tempfile::TempDir,
    reads: BTreeMap<Vec<Option<Vec<u8>>>, Read>,
}

impl Domain {
    fn repository(&self, entries: &Entries) -> (Arc<MemoryStateStorage>, Repository) {
        let persistence = Arc::new(Recorded::default());
        memory_repository_over(
            self.root.path(),
            StateView::new(entries.clone()),
            persistence,
        )
    }

    /// The state after a landing of `run` with `--replace`, or `None` when
    /// the landing refuses the run because it changes asserted evidence or
    /// adds an immutable record again.
    fn land(&self, entries: &Entries, run: &Value) -> Option<Entries> {
        let (storage, repository) = self.repository(entries);
        let batch: IdeationLandingBatch = serde_json::from_value(run.clone()).unwrap();
        if let Err(error) = repository
            .store()
            .land_ideation_batch(&scope(), batch, true)
        {
            let text = error.to_string();
            assert!(
                text.contains("cannot be replaced") || text.contains("is immutable"),
                "{text}"
            );
            return None;
        }
        Some(storage.read_view().unwrap().into_entries())
    }

    /// The read of each record of the ideation files, after the check of the
    /// store passes.
    fn read(&mut self, entries: &Entries) -> Read {
        let files: Vec<_> = FILES
            .iter()
            .map(|file| entries.get(&file_key(file)).cloned())
            .collect();
        if let Some(read) = self.reads.get(&files) {
            return read.clone();
        }
        let (_, repository) = self.repository(entries);
        let store = repository.store();
        store.validate_ideation_scope(&scope()).unwrap();
        let read = [
            (
                "contributions",
                values(&store.list_contributions(&scope()).unwrap()),
            ),
            (
                "synthesis_packets",
                values(&store.list_synthesis_packets(&scope()).unwrap()),
            ),
            (
                "proposal_cards",
                values(&store.list_proposal_definitions(&scope()).unwrap()),
            ),
            (
                "assertions",
                values(&store.list_assertion_records(&scope()).unwrap()),
            ),
            (
                "dispositions",
                values(&store.list_dispositions(&scope()).unwrap()),
            ),
        ]
        .into_iter()
        .flat_map(|(file, records)| {
            records
                .into_iter()
                .map(move |record| ((file, record["id"].as_str().unwrap().to_owned()), record))
        })
        .collect::<Read>();
        self.reads.insert(files, read.clone());
        read
    }
}

fn values<T: serde::Serialize>(records: &[T]) -> Vec<Value> {
    records
        .iter()
        .map(|record| serde_json::to_value(record).unwrap())
        .collect()
}

fn rows(entries: &Entries, file: &str) -> Vec<Value> {
    entries.get(&file_key(file)).map_or_else(Vec::new, |bytes| {
        serde_json::Deserializer::from_slice(bytes)
            .into_iter::<Value>()
            .map(Result::unwrap)
            .collect()
    })
}

/// The merge of each ideation file by its record merge, and the ids of the
/// conflicts. The result holds the other files of `ours`.
fn merge(base: &Entries, ours: &Entries, theirs: &Entries) -> (Entries, BTreeSet<String>) {
    let mut merged = ours.clone();
    let mut conflicts = BTreeSet::new();
    for file in FILES {
        let sides = [base, ours, theirs].map(|side| rows(side, file));
        let records = match merge_records("id", &sides[0], &sides[1], &sides[2]).unwrap() {
            MergeOutcome::Clean { records } => records,
            MergeOutcome::Conflicted {
                conflicts: each,
                partial,
            } => {
                conflicts.extend(each.into_iter().map(|conflict| conflict.record_id));
                partial
            }
        };
        if [base, ours, theirs]
            .iter()
            .any(|side| side.contains_key(&file_key(file)))
        {
            let mut bytes = Vec::new();
            for record in records {
                bytes.extend(serde_json::to_vec(&record).unwrap());
                bytes.push(b'\n');
            }
            merged.insert(file_key(file), bytes);
        }
    }
    (merged, conflicts)
}

/// The read and the conflict ids that the rule of this test gives.
fn rule(base: &Read, ours: &Read, theirs: &Read) -> (Read, BTreeSet<String>) {
    let mut read = Read::new();
    let mut conflicts = BTreeSet::new();
    let keys: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    for record in keys {
        let (b, o, t) = (base.get(record), ours.get(record), theirs.get(record));
        let value = if o == t || t == b {
            o
        } else if o == b {
            t
        } else {
            conflicts.insert(record.1.clone());
            o
        };
        if let Some(value) = value {
            read.insert(record.clone(), value.clone());
        }
    }
    (read, conflicts)
}

/// Merges `ours` and `theirs` on `base` and asserts the rule of this test.
fn assert_merge(
    domain: &mut Domain,
    base: &Entries,
    ours: &Entries,
    theirs: &Entries,
) -> (Entries, bool) {
    let (merged, conflicts) = merge(base, ours, theirs);
    let [b, o, t] = [base, ours, theirs].map(|state| domain.read(state));
    let expected = rule(&b, &o, &t);
    assert_eq!((domain.read(&merged), conflicts.clone()), expected);
    (merged, conflicts.is_empty())
}

/// Each state that a side reaches with up to `length` runs of the pool.
fn histories(domain: &Domain, base: &Entries, length: usize) -> Vec<Entries> {
    let mut states = vec![base.clone()];
    let mut last = vec![base.clone()];
    for _ in 0..length {
        last = last
            .iter()
            .flat_map(|state| {
                pool()
                    .iter()
                    .filter_map(|run| domain.land(state, run))
                    .collect::<Vec<_>>()
            })
            .collect();
        states.extend(last.iter().cloned());
    }
    states
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", conformance)]
#[verifies("rule_record_merge", conformance)]
fn each_merge_of_swarm_runs_follows_the_record_rule() {
    let mut domain = Domain {
        root: tempfile::tempdir().unwrap(),
        reads: BTreeMap::new(),
    };
    let initial = crate::memory::initial_state().into_entries();
    let base = domain
        .land(
            &initial,
            &json!({"contributions": [contribution("c", "Base.")]}),
        )
        .unwrap();
    let sides = histories(&domain, &base, 2);
    let singles = histories(&domain, &base, 1);
    assert!(sides.len() > singles.len() && singles.len() == 5);
    let pool = pool();
    for ours in &sides {
        for theirs in &sides {
            let (merged, clean) = assert_merge(&mut domain, &base, ours, theirs);
            let (reverse, _) = assert_merge(&mut domain, &base, theirs, ours);
            if !clean {
                continue;
            }
            assert!(
                merged == reverse,
                "a clean merge gives the same files in the two directions"
            );
            if !(singles.contains(ours) && singles.contains(theirs)) {
                continue;
            }
            for third in &singles {
                assert_merge(&mut domain, &base, &merged, third);
            }
            let ones: Vec<_> = pool
                .iter()
                .filter_map(|run| domain.land(&merged, run))
                .collect();
            let twos: Vec<_> = pool
                .iter()
                .filter_map(|run| domain.land(&reverse, run))
                .collect();
            for one in &ones {
                for two in &twos {
                    assert_merge(&mut domain, &merged, one, two);
                }
            }
        }
    }
}
