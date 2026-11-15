//! The merge of the landings file for each history of a small domain, in
//! each direction, against a record rule that this test states by itself.
//!
//! The base holds one batch. Each side adds zero to three batches of a pool:
//! a batch for each of two contribution ids and two contents, and one batch
//! with the two ids. A batch that the two sides add with equal rows, or that
//! the base holds, is a common batch. A side changes each record of a batch
//! that only that side holds. A record that only one side changes has the
//! read of that side; a record that the two sides change is a conflict when
//! the two reads are different. The reads come from the store.

use crate::memory::{initial_state, key, memory_repository_over, scope, shard, Recorded};
use provenance_macros::verifies;
use provenance_store::merge::{merge_records, MergeOutcome, RowIdentity};
use provenance_store::storage::StateView;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

const LANDINGS: &str = "scopes/default/ideation/landings.jsonl";

/// The record of each contribution id of a read.
type Read = BTreeMap<String, Value>;

fn contribution(id: &str, finding: &str) -> Value {
    json!({
        "scope_id": "default", "id": id,
        "target": {"artifact_type": "requirement", "artifact_id": "req_expiry"},
        "participant_slot": "slot_a", "stance": "support", "strongest_finding": finding,
        "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
        "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct."}, "open_questions": []
    })
}

fn batch(records: &[(&str, &str)]) -> Value {
    let contributions: Vec<_> = records
        .iter()
        .map(|(id, finding)| contribution(id, finding))
        .collect();
    json!({ "contributions": contributions })
}

/// The reads of the store, one for each file.
struct Reads {
    root: tempfile::TempDir,
    done: HashMap<Vec<Value>, Read>,
}

impl Reads {
    /// The read of a landings file with `rows`. The store also checks the
    /// file as `provenance check` does.
    fn of(&mut self, rows: &[Value]) -> Read {
        if let Some(read) = self.done.get(rows) {
            return read.clone();
        }
        let mut entries = initial_state().into_entries();
        entries.insert(key(LANDINGS), shard(rows));
        let persistence = Arc::new(Recorded::default());
        let (_, repository) =
            memory_repository_over(self.root.path(), StateView::new(entries), persistence);
        let store = repository.store();
        store.validate_ideation_scope(&scope()).unwrap();
        let read: Read = store
            .list_contributions(&scope())
            .unwrap()
            .into_iter()
            .map(|record| {
                let value = serde_json::to_value(&record).unwrap();
                (record.id.as_str().to_owned(), value)
            })
            .collect();
        self.done.insert(rows.to_vec(), read.clone());
        read
    }
}

/// For each row of `side`, whether it is a common batch: the n-th row of
/// `side` equal to a row is common when the base holds n such rows, or the
/// other side holds n such rows too.
fn common_rows(base: &[Value], side: &[Value], other: &[Value]) -> Vec<bool> {
    let count = |rows: &[Value], row: &Value| rows.iter().filter(|each| *each == row).count();
    side.iter()
        .enumerate()
        .map(|(index, row)| {
            let n = count(&side[..=index], row);
            n <= count(base, row) || n <= count(other, row)
        })
        .collect()
}

/// The ids of the records of the rows of `side` that are not common.
fn changes(side: &[Value], common: &[bool]) -> Vec<String> {
    side.iter()
        .zip(common)
        .filter(|(_, common)| !**common)
        .flat_map(|(row, _)| row["contributions"].as_array().unwrap().iter())
        .map(|record| record["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Whether `rows` holds `part` in its sequence.
fn holds_in_sequence(rows: &[Value], part: &[Value]) -> bool {
    let mut rows = rows.iter();
    part.iter().all(|row| rows.any(|each| each == row))
}

/// What one merge gives, for the comparison of the two directions.
#[derive(Debug, PartialEq)]
enum Merged {
    Clean(Read),
    Conflict,
}

/// Merges `ours` and `theirs` and checks each assertion of the domain.
fn merge(reads: &mut Reads, base: &[Value], ours: &[Value], theirs: &[Value]) -> Merged {
    let outcome = merge_records(RowIdentity::Batch, base, ours, theirs).unwrap();
    let [our_common, their_common] = [
        common_rows(base, ours, theirs),
        common_rows(base, theirs, ours),
    ];
    let [our_read, their_read] = [reads.of(ours), reads.of(theirs)];
    let [our_changes, their_changes] = [changes(ours, &our_common), changes(theirs, &their_common)];
    let record_conflict = our_changes
        .iter()
        .any(|id| their_changes.contains(id) && our_read.get(id) != their_read.get(id));
    let sequence = |side: &[Value], common: &[bool]| -> Vec<Value> {
        side.iter()
            .zip(common)
            .filter(|(_, common)| **common)
            .map(|(row, _)| row.clone())
            .collect()
    };
    let one_sequence = sequence(ours, &our_common) == sequence(theirs, &their_common);
    match outcome {
        MergeOutcome::Clean { records } => {
            assert!(!record_conflict && one_sequence);
            let mut expected = our_read.clone();
            for (id, record) in &their_read {
                if their_changes.contains(id) || !our_changes.contains(id) {
                    expected.insert(id.clone(), record.clone());
                }
            }
            let read = reads.of(&records);
            assert_eq!(read, expected);
            assert!(holds_in_sequence(&records, ours) && holds_in_sequence(&records, theirs));
            let common = our_common.iter().filter(|common| **common).count();
            assert_eq!(records.len(), ours.len() + theirs.len() - common);
            Merged::Clean(read)
        }
        conflicted @ MergeOutcome::Conflicted { .. } => {
            assert!(record_conflict || !one_sequence);
            let report = serde_json::to_value(&conflicted).unwrap();
            let MergeOutcome::Conflicted { partial, .. } = conflicted else {
                unreachable!()
            };
            assert_eq!(partial, ours);
            let rows: Vec<_> = report["left_out"]
                .as_array()
                .map(|left_out| left_out.iter().map(|batch| batch["row"].clone()).collect())
                .unwrap_or_default();
            let held_by_ours = common_rows(&[], theirs, ours);
            let only_theirs: Vec<_> = (1..=theirs.len())
                .filter(|row| !held_by_ours[row - 1])
                .map(Value::from)
                .collect();
            assert_eq!(rows, only_theirs);
            Merged::Conflict
        }
    }
}

#[test]
#[verifies("rule_landing_batch_merge", exhaustion)]
#[verifies("rule_landing_batch_merge_conflict", exhaustion)]
fn each_history_of_a_small_domain_merges_as_the_record_rule_gives_in_each_direction() {
    let pool = [
        batch(&[("contrib_c", "X")]),
        batch(&[("contrib_c", "Y")]),
        batch(&[("contrib_d", "X")]),
        batch(&[("contrib_d", "Y")]),
        batch(&[("contrib_c", "X"), ("contrib_d", "X")]),
    ];
    let base = vec![pool[0].clone()];
    let mut additions = vec![Vec::new()];
    for length in 1..=3 {
        let longer: Vec<Vec<Value>> = additions
            .iter()
            .filter(|added: &&Vec<Value>| added.len() == length - 1)
            .flat_map(|added| {
                pool.iter().map(move |row| {
                    let mut added = added.clone();
                    added.push(row.clone());
                    added
                })
            })
            .collect();
        additions.extend(longer);
    }
    let side = |added: &Vec<Value>| [base.clone(), added.clone()].concat();
    let mut reads = Reads {
        root: tempfile::tempdir().unwrap(),
        done: HashMap::new(),
    };
    let (mut histories, mut clean) = (0, 0);
    for our_added in &additions {
        for their_added in &additions {
            let (ours, theirs) = (side(our_added), side(their_added));
            let into_ours = merge(&mut reads, &base, &ours, &theirs);
            let into_theirs = merge(&mut reads, &base, &theirs, &ours);
            assert_eq!(into_ours, into_theirs, "ours {ours:?}, theirs {theirs:?}");
            histories += 1;
            clean += usize::from(matches!(into_ours, Merged::Clean(_)));
        }
    }
    println!("histories {histories}, clean {clean}, conflicts {}", histories - clean);
    assert_eq!((histories, clean), (24_336, 10_618));
}
