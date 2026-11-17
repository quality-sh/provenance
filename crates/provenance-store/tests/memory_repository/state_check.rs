//! The check of a state over the memory adapter, with and without `host`.
//!
//! Each case plants rows in the state and expects the finding line that
//! `provenance check` reports for the same rows.

use super::memory::{key, memory_repository_with, shard};
use provenance_macros::verifies;
use provenance_store::state_check::{check_state, StateFindings};
use provenance_store::storage::StateKey;
use serde_json::{json, Value};

/// The findings of a repository-wide check of the initial state with `rows`,
/// run in one publication section as `provenance check` runs it. The check
/// writes no file and gives the adapter no write set.
fn findings(rows: &[(&str, Vec<Value>)]) -> StateFindings {
    let memory = memory_repository_with(
        rows.iter()
            .map(|(path, rows)| (key(path), shard(rows)))
            .collect::<Vec<(StateKey, Vec<u8>)>>(),
    );
    let found = memory
        .repository
        .store()
        .with_repository_publication(|held| check_state(held, &held.manifest()?, true))
        .unwrap();
    memory.assert_empty();
    assert!(memory.recorded.0.lock().unwrap().is_empty());
    found
}

fn implementation_binding(scope: &str, id: &str, rule_id: &str) -> Value {
    json!({
        "scope_id": scope,
        "id": id,
        "rule_id": rule_id,
        "declared_by": "spec://typescript/workflows",
        "file": "src/runtime.ts",
        "symbol": "startWorkflow",
    })
}

fn verification_binding(scope: &str, id: &str, rule_id: &str) -> Value {
    json!({
        "scope_id": scope,
        "id": id,
        "rule_id": rule_id,
        "key": "share-link-expiry",
        "method": "examples",
        "declared_by": "test://typescript",
        "file": "tests/expiry.test.ts",
    })
}

fn source(scope: &str, id: &str, origin_thread: &str, origin_message: &str) -> Value {
    json!({
        "scope_id": scope,
        "id": id,
        "name": "Policy",
        "source_type": "policy",
        "origin_thread": origin_thread,
        "origin_message": origin_message,
    })
}

fn thread(scope: &str, id: &str, parent: &str) -> Value {
    json!({
        "scope_id": scope,
        "id": id,
        "parent": {"node_type": "requirement", "node_id": parent},
        "status": "resolved",
        "created_at": 1,
    })
}

fn message(scope: &str, id: &str, thread_id: &str) -> Value {
    json!({
        "scope_id": scope,
        "id": id,
        "thread_id": thread_id,
        "role": "user",
        "actor": "agent",
        "body": "A message",
        "created_at": 1,
    })
}

fn proposal(scope: &str, id: &str, target: &str, source_id: &str) -> Value {
    json!({
        "scope_id": scope,
        "id": id,
        "proposal_key": id,
        "proposal_type": "source_gap",
        "title": "A",
        "summary": "A",
        "traceability": {
            "target": {"artifact_type": "source", "artifact_id": target},
            "source_ids": [source_id],
            "evidence_references": [],
            "supporting_claim_ids": [],
        },
        "promotion_state": "proposed",
    })
}

#[test]
#[verifies("rule_state_check_without_host", examples)]
fn a_clean_state_gives_no_finding() {
    let found = findings(&[]);

    assert_eq!(found, StateFindings::default());
    assert_eq!(found.message(), None);
}

#[test]
#[verifies("rule_state_check_without_host", examples)]
fn each_dangling_reference_gives_the_line_of_provenance_check() {
    let cases = [
        (
            "scopes/default/implementations/binding.jsonl",
            implementation_binding("default", "implementation_binding_missing", "rule_missing"),
            "implementation binding implementation_binding_missing has dangling reference: rule_id rule rule_missing",
        ),
        (
            "scopes/default/verifications/binding.jsonl",
            verification_binding("default", "verification_binding_missing", "rule_missing"),
            "verification binding verification_binding_missing has dangling reference: rule_id rule rule_missing",
        ),
        (
            "scopes/default/threads/2026-07.jsonl",
            message("default", "message_orphan", "thread_missing"),
            "message message_orphan has dangling reference: thread thread thread_missing",
        ),
        (
            "scopes/default/threads/threads.jsonl",
            thread("missing", "thread_bad_scope", "req_expiry"),
            "thread thread_bad_scope is in unknown scope missing",
        ),
    ];
    for (path, row, line) in cases {
        let found = findings(&[(path, vec![row])]);

        assert_eq!(found.dangling, [line], "{path}");
        assert_eq!(
            found.message().as_deref(),
            Some(format!("dangling reference(s):\n- {line}").as_str()),
        );
    }
}

#[test]
#[verifies("rule_state_check_without_host", examples)]
fn origins_and_proposal_references_give_one_line_each() {
    let found = findings(&[
        (
            "scopes/default/sources/source.jsonl",
            vec![source(
                "default",
                "source_policy",
                "thread_missing",
                "message_missing",
            )],
        ),
        (
            "scopes/default/ideation/proposal_cards.jsonl",
            vec![proposal(
                "default",
                "proposal_a",
                "source_absent",
                "source_gone",
            )],
        ),
    ]);

    assert_eq!(
        found.dangling,
        [
            "source source_policy has dangling reference: origin_thread thread thread_missing",
            "source source_policy has dangling reference: origin_message message message_missing",
            "proposal proposal_a has dangling reference: target source source_absent",
            "proposal proposal_a has dangling reference: source_id source source_gone",
        ]
    );
}

#[test]
#[verifies("rule_state_check_without_host", examples)]
fn findings_keep_the_order_of_scopes_and_then_of_families() {
    let found = findings(&[
        (
            "scopes/imported/implementations/binding.jsonl",
            vec![implementation_binding(
                "imported",
                "implementation_binding_imported",
                "rule_gone",
            )],
        ),
        (
            "scopes/imported/threads/2026-07.jsonl",
            vec![message("imported", "message_imported", "thread_gone")],
        ),
        (
            "scopes/default/ideation/proposal_cards.jsonl",
            vec![proposal(
                "default",
                "proposal_a",
                "source_absent",
                "source_gone",
            )],
        ),
        (
            "scopes/default/threads/2026-07.jsonl",
            vec![message("default", "message_default", "thread_gone")],
        ),
        (
            "scopes/default/implementations/binding.jsonl",
            vec![implementation_binding(
                "default",
                "implementation_binding_default",
                "rule_gone",
            )],
        ),
    ]);

    assert_eq!(
        found.dangling,
        [
            "implementation binding implementation_binding_default has dangling reference: rule_id rule rule_gone",
            "message message_default has dangling reference: thread thread thread_gone",
            "proposal proposal_a has dangling reference: target source source_absent",
            "proposal proposal_a has dangling reference: source_id source source_gone",
            "implementation binding implementation_binding_imported has dangling reference: rule_id rule rule_gone",
            "message message_imported has dangling reference: thread thread thread_gone",
        ]
    );
}

#[test]
#[verifies("rule_state_check_without_host", examples)]
fn a_review_row_that_a_read_refuses_gives_the_line_of_provenance_check() {
    let found = findings(&[class_rows(Class::ReviewRow)]);

    assert_eq!(
        found,
        StateFindings {
            review_rows: vec![REVIEW_ROW.into()],
            ..StateFindings::default()
        }
    );
    assert_eq!(
        found.message().as_deref(),
        Some(format!("review row finding(s):\n- {REVIEW_ROW}").as_str()),
    );
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    ScopeOwnership,
    ReviewRow,
    ScopeDirectory,
    Dangling,
}

const SCOPE_OWNERSHIP: &str =
    "source source_misfiled loaded from scope imported has embedded scope_id default";
const REVIEW_ROW: &str = "scopes/default/ideation/withdrawals.jsonl line 1: withdrawal \
    withdrawal_lost names no review submission: proposal_missing; To repair, remove \
    ideation/withdrawals.jsonl line 1, then commit.";
const SCOPE_DIRECTORY: &str = "scope directory unlisted is absent from manifest";
const DANGLING: &str =
    "message message_orphan has dangling reference: thread thread thread_missing";

/// The rows that give one finding of `class`.
fn class_rows(class: Class) -> (&'static str, Vec<Value>) {
    let (path, row) = match class {
        Class::ScopeOwnership => (
            "scopes/imported/sources/source.jsonl",
            json!({"scope_id": "default", "id": "source_misfiled", "name": "Misfiled",
                "source_type": "document"}),
        ),
        Class::ReviewRow => (
            "scopes/default/ideation/withdrawals.jsonl",
            json!({"scope_id": "default", "id": "withdrawal_lost",
                "proposal_id": "proposal_missing", "actor": {"identity_type": "agent", "id": "agent"}, "reason": "Lost."}),
        ),
        Class::ScopeDirectory => (
            "scopes/unlisted/requirements/req.jsonl",
            json!({"scope_id": "unlisted", "id": "req_unlisted", "statement": "Unlisted",
                "status": "active"}),
        ),
        Class::Dangling => (
            "scopes/default/threads/2026-07.jsonl",
            message("default", "message_orphan", "thread_missing"),
        ),
    };
    (path, vec![row])
}

/// The findings that hold the one finding of each class in `classes`.
fn holding(classes: &[Class]) -> StateFindings {
    let line = |class: Class, text: &str| {
        if classes.contains(&class) {
            vec![text.to_owned()]
        } else {
            Vec::new()
        }
    };
    StateFindings {
        scope_ownership: line(Class::ScopeOwnership, SCOPE_OWNERSHIP),
        review_rows: line(Class::ReviewRow, REVIEW_ROW),
        scope_directories: line(Class::ScopeDirectory, SCOPE_DIRECTORY),
        dangling: line(Class::Dangling, DANGLING),
    }
}

/// The text that `provenance check` reports when `class` wins.
fn reported(class: Class) -> String {
    let (title, line) = match class {
        Class::ScopeOwnership => ("scope ownership finding(s)", SCOPE_OWNERSHIP),
        Class::ReviewRow => ("review row finding(s)", REVIEW_ROW),
        Class::ScopeDirectory => ("scope directory finding(s)", SCOPE_DIRECTORY),
        Class::Dangling => ("dangling reference(s)", DANGLING),
    };
    format!("{title}:\n- {line}")
}

/// Each pair of finding classes, in the order in which `provenance check`
/// reports them, and the classes that the check reached. The first class of
/// the pair wins. A scope-ownership finding stops the check before the review
/// rows and the references; a review-row finding stops it before the
/// references.
#[test]
#[verifies("rule_state_check_without_host", examples)]
fn each_pair_of_finding_classes_gives_the_class_of_provenance_check() {
    use Class::{Dangling, ReviewRow, ScopeDirectory, ScopeOwnership};
    let cases: [([Class; 2], &[Class]); 6] = [
        ([ScopeOwnership, ReviewRow], &[ScopeOwnership]),
        (
            [ScopeOwnership, ScopeDirectory],
            &[ScopeOwnership, ScopeDirectory],
        ),
        ([ScopeOwnership, Dangling], &[ScopeOwnership]),
        ([ReviewRow, ScopeDirectory], &[ReviewRow, ScopeDirectory]),
        ([ReviewRow, Dangling], &[ReviewRow]),
        ([ScopeDirectory, Dangling], &[ScopeDirectory, Dangling]),
    ];
    for (pair, reached) in cases {
        let found = findings(&pair.map(class_rows));

        assert_eq!(found, holding(reached), "{pair:?}");
        assert_eq!(found.message(), Some(reported(pair[0])), "{pair:?}");
    }
}
