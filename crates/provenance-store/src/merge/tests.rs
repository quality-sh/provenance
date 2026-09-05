use provenance_macros::verifies;

use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

fn record(id: &str, statement: &str) -> Value {
    serde_json::json!({ "id": id, "statement": statement })
}

fn ids(records: &[Value]) -> Vec<&str> {
    records
        .iter()
        .map(|record| record.get("id").unwrap().as_str().unwrap())
        .collect()
}

/// What one side holds for one record id: nothing, or a value belonging to
/// an equality class. Two slots hold equal values exactly when their class
/// labels match, so a `[Slot; 3]` names one whole case of the merge domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Absent,
    Present(u8),
}

/// Every way to label `len` slots by equality class, up to renaming of the
/// classes: the restricted growth strings of length `len`. Each string is a
/// distinct partition of the slots, so the list is complete and has no
/// duplicates.
fn equality_patterns(len: usize) -> Vec<Vec<u8>> {
    let mut patterns: Vec<Vec<u8>> = vec![Vec::new()];
    for _ in 0..len {
        let mut grown = Vec::new();
        for pattern in patterns {
            let ceiling = pattern.iter().copied().max().map_or(0, |max| max + 1);
            for label in 0..=ceiling {
                let mut extended = pattern.clone();
                extended.push(label);
                grown.push(extended);
            }
        }
        patterns = grown;
    }
    patterns
}

/// The whole finite domain of a single record id: each of (base, ours,
/// theirs) is absent or present, crossed with every equality pattern over
/// the present slots.
fn every_case() -> Vec<[Slot; 3]> {
    let mut cases = Vec::new();
    for presence in 0u8..8 {
        let present: Vec<usize> = (0..3).filter(|slot| presence >> slot & 1 == 1).collect();
        for pattern in equality_patterns(present.len()) {
            let mut case = [Slot::Absent; 3];
            for (&slot, &label) in present.iter().zip(pattern.iter()) {
                case[slot] = Slot::Present(label);
            }
            cases.push(case);
        }
    }
    cases
}

/// Every case except the one where no side holds the record: an id absent
/// from base, ours and theirs is never merged, because the id set is built
/// from the three inputs.
fn reachable_cases() -> Vec<[Slot; 3]> {
    let cases = every_case();
    assert_eq!(cases.len(), 15, "case enumeration changed size");
    let reachable: Vec<[Slot; 3]> = cases
        .into_iter()
        .filter(|case| *case != [Slot::Absent; 3])
        .collect();
    assert_eq!(reachable.len(), 14, "expected 14 reachable cases");
    reachable
}

fn slot_record(id: &str, slot: Slot) -> Option<Value> {
    match slot {
        Slot::Absent => None,
        Slot::Present(class) => Some(record(id, &format!("v{class}"))),
    }
}

/// Independent restatement of the decision, written from its own words and
/// not from `merge_records`:
///
/// keep what only one side touched, keep an identical change once, take the
/// side that moved when the other stood still, and when both moved
/// differently keep ours and report the clash; a record one side deleted
/// while the other edited is kept and reported.
///
/// Absence is treated as just another thing a side can hold, so a deletion
/// is a move like any other and the whole decision is three comparisons.
///
/// The clash is named by what the base held: nothing at all is an add/add,
/// a record both sides moved away from is a divergent edit, and a move
/// against a deletion is a delete/modify.
fn decide(
    base: Option<&Value>,
    ours: Option<&Value>,
    theirs: Option<&Value>,
) -> (Option<Value>, Option<MergeConflictKind>) {
    if ours == theirs {
        // Neither side touched it, or both made the same change: keep once.
        return (ours.cloned(), None);
    }
    if ours == base {
        // Only theirs moved.
        return (theirs.cloned(), None);
    }
    if theirs == base {
        // Only ours moved.
        return (ours.cloned(), None);
    }
    // Both moved, and differently.
    let kind = if ours.is_none() || theirs.is_none() {
        MergeConflictKind::DeleteModify
    } else if base.is_none() {
        MergeConflictKind::AddAdd
    } else {
        MergeConflictKind::DivergentEdit
    };
    (ours.or(theirs).cloned(), Some(kind))
}

/// The oracle for one id: the record the merge should keep, and the
/// conflict it should report.
fn expected(id: &str, case: [Slot; 3]) -> (Option<Value>, Option<MergeConflict>) {
    let base = slot_record(id, case[0]);
    let ours = slot_record(id, case[1]);
    let theirs = slot_record(id, case[2]);
    let (kept, kind) = decide(base.as_ref(), ours.as_ref(), theirs.as_ref());
    let conflict = kind.map(|kind| MergeConflict {
        kind,
        record_id: id.to_string(),
        base,
        ours,
        theirs,
    });
    (kept, conflict)
}

fn parts(outcome: MergeOutcome<Vec<Value>>) -> (Vec<Value>, Vec<MergeConflict>) {
    match outcome {
        MergeOutcome::Clean { records } => (records, Vec::new()),
        MergeOutcome::Conflicted { conflicts, partial } => (partial, conflicts),
    }
}

#[test]
#[verifies("rule_record_merge", exhaustion)]
fn every_reachable_single_record_case_matches_the_decision() {
    for case in reachable_cases() {
        let id = "rec";
        let base: Vec<Value> = slot_record(id, case[0]).into_iter().collect();
        let ours: Vec<Value> = slot_record(id, case[1]).into_iter().collect();
        let theirs: Vec<Value> = slot_record(id, case[2]).into_iter().collect();

        let outcome = merge_records(&base, &ours, &theirs).unwrap();
        let clean = matches!(outcome, MergeOutcome::Clean { .. });
        let (records, conflicts) = parts(outcome);

        let (kept, conflict) = expected(id, case);
        assert_eq!(
            records,
            kept.into_iter().collect::<Vec<Value>>(),
            "kept record disagrees with the decision for case {case:?}"
        );
        assert_eq!(
            conflicts,
            conflict.clone().into_iter().collect::<Vec<MergeConflict>>(),
            "reported conflict disagrees with the decision for case {case:?}"
        );
        assert_eq!(
            clean,
            conflict.is_none(),
            "merge status disagrees with the decision for case {case:?}"
        );
    }
}

#[test]
#[verifies("rule_record_merge", exhaustion)]
fn only_the_add_add_case_reports_the_new_kind() {
    // Ten of the 14 reachable cases merge cleanly. The four that clash are, in
    // enumeration order: ours edited what theirs deleted, theirs edited what
    // ours deleted, both sides added the same id with different content, and
    // both sides moved a record that was in the base. Splitting `AddAdd` out
    // moves the third of those and nothing else - every other case reports
    // what it reported before.
    let kinds: Vec<MergeConflictKind> = reachable_cases()
        .into_iter()
        .filter_map(|case| expected("rec", case).1.map(|conflict| conflict.kind))
        .collect();

    assert_eq!(
        kinds,
        vec![
            MergeConflictKind::DeleteModify,
            MergeConflictKind::DeleteModify,
            MergeConflictKind::AddAdd,
            MergeConflictKind::DivergentEdit,
        ]
    );
}

#[test]
fn jsonl_merge_input_rejects_an_unsupported_record_version_before_loading() {
    let directory = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(directory.path().join("rule.jsonl")).unwrap();
    std::fs::write(
        &path,
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1, "id": "rule_future"})
            .to_string()
            + "\n",
    )
    .unwrap();

    let error = read_jsonl_records(&path).unwrap_err().to_string();

    assert!(error.contains("rule.jsonl line 1"), "{error}");
    assert!(error.contains("record rule_future"), "{error}");
    assert!(
        error.contains(&format!(
            "has schema_version {}",
            SUPPORTED_SCHEMA_VERSION.0 + 1
        )),
        "{error}"
    );
}

#[test]
fn jsonl_merge_input_rejects_an_unsupported_nested_landing_record() {
    let directory = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(
        directory
            .path()
            .join(".provenance/state/scopes/default/ideation/landings.jsonl"),
    )
    .unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::json!({
            "contributions": [{"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1, "id": "contribution_future"}]
        })
        .to_string()
            + "\n",
    )
    .unwrap();

    let error = read_jsonl_records(&path).unwrap_err().to_string();

    assert!(error.contains("record contribution_future"), "{error}");
    assert!(
        error.contains(&format!(
            "schema_version {}",
            SUPPORTED_SCHEMA_VERSION.0 + 1
        )),
        "{error}"
    );
}

/// Deterministic generator; the property test needs varied inputs, not
/// random ones, and the crate takes no test-only dependencies.
struct Generator(u64);

impl Generator {
    fn pick(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let bound = u64::try_from(bound).unwrap();
        usize::try_from((self.0 >> 33) % bound).unwrap()
    }

    fn shuffle(&mut self, records: &mut [Value]) {
        for index in (1..records.len()).rev() {
            records.swap(index, self.pick(index + 1));
        }
    }
}

#[test]
#[verifies("rule_record_merge", property)]
fn multi_record_merge_is_the_per_record_decision_applied_independently() {
    // Property: for a file of many records, the merge keeps exactly what
    // the per-record decision keeps for each id on its own, reports exactly
    // the conflicts the per-record decision reports, orders both by id, and
    // does not depend on the order the records arrive in. No id may affect
    // the outcome of another.
    let cases = reachable_cases();
    let pool = ["rec_e", "rec_a", "rec_d", "rec_b", "rec_c", "rec_f"];
    let mut generator = Generator(0x5eed_1234_9abc_def0);

    for trial in 0..500 {
        let mut chosen: Vec<(&str, [Slot; 3])> = Vec::new();
        for id in pool {
            if generator.pick(3) != 0 {
                chosen.push((id, cases[generator.pick(cases.len())]));
            }
        }
        if chosen.is_empty() {
            continue;
        }

        let mut sides: [Vec<Value>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for &(id, case) in &chosen {
            for (side, records) in sides.iter_mut().enumerate() {
                records.extend(slot_record(id, case[side]));
            }
        }
        for records in &mut sides {
            generator.shuffle(records);
        }

        chosen.sort_by_key(|&(id, _)| id);
        let mut wanted_records = Vec::new();
        let mut wanted_conflicts = Vec::new();
        for &(id, case) in &chosen {
            let (kept, conflict) = expected(id, case);
            wanted_records.extend(kept);
            wanted_conflicts.extend(conflict);
        }

        let outcome = merge_records(&sides[0], &sides[1], &sides[2]).unwrap();
        let clean = matches!(outcome, MergeOutcome::Clean { .. });
        let (records, conflicts) = parts(outcome);

        assert_eq!(records, wanted_records, "trial {trial}: kept records");
        assert_eq!(conflicts, wanted_conflicts, "trial {trial}: conflicts");
        assert_eq!(
            clean,
            wanted_conflicts.is_empty(),
            "trial {trial}: merge status"
        );
    }
}

#[test]
#[verifies("rule_record_merge", examples)]
fn merge_keeps_one_sided_additions_and_sorts_by_stable_key() {
    let merged = merge_records(&[], &[record("rule_b", "b")], &[record("rule_a", "a")])
        .unwrap()
        .unwrap_clean();

    assert_eq!(ids(&merged), vec!["rule_a", "rule_b"]);
}

#[test]
#[verifies("rule_record_merge", examples)]
fn merge_collapses_identical_edits() {
    let base = [record("rule_a", "old")];
    let ours = [record("rule_a", "new")];
    let theirs = [record("rule_a", "new")];

    let merged = merge_records(&base, &ours, &theirs).unwrap().unwrap_clean();

    assert_eq!(merged, vec![record("rule_a", "new")]);
}

#[test]
#[verifies("rule_record_merge", examples)]
fn merge_reports_divergent_edits_with_the_base_pre_image() {
    let base = [record("rule_a", "old")];
    let ours = [record("rule_a", "ours")];
    let theirs = [record("rule_a", "theirs")];

    let conflicts = merge_records(&base, &ours, &theirs)
        .unwrap()
        .unwrap_conflicts();

    assert_eq!(conflicts[0].kind, MergeConflictKind::DivergentEdit);
    assert_eq!(conflicts[0].record_id, "rule_a");
    assert_eq!(conflicts[0].base, Some(record("rule_a", "old")));
}

#[test]
#[verifies("rule_record_merge", examples)]
fn merge_reports_add_add_separately_from_divergent_edits() {
    let ours = [record("rule_a", "ours")];
    let theirs = [record("rule_a", "theirs")];

    let conflicts = merge_records(&[], &ours, &theirs)
        .unwrap()
        .unwrap_conflicts();

    assert_eq!(conflicts[0].kind, MergeConflictKind::AddAdd);
    assert_eq!(conflicts[0].record_id, "rule_a");
    assert_eq!(conflicts[0].base, None, "an add/add has no base pre-image");
    assert_eq!(conflicts[0].ours, Some(record("rule_a", "ours")));
    assert_eq!(conflicts[0].theirs, Some(record("rule_a", "theirs")));
}

#[test]
#[verifies("rule_record_merge", examples)]
fn merge_reports_delete_modify_conflict() {
    let base = [record("rule_a", "old")];
    let theirs = [record("rule_a", "new")];

    let conflicts = merge_records(&base, &[], &theirs)
        .unwrap()
        .unwrap_conflicts();

    assert_eq!(conflicts[0].kind, MergeConflictKind::DeleteModify);
    assert_eq!(conflicts[0].record_id, "rule_a");
    assert_eq!(conflicts[0].base, Some(record("rule_a", "old")));
    assert_eq!(conflicts[0].ours, None);
}
