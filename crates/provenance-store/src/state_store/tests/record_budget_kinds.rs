//! Each record kind and each binding at the record budget: the writer accepts
//! a row of exactly the budget and refuses a row of one byte more, with the
//! record, the size of its row and the budget.

use super::seeded_requirement_store;
use crate::state_store::{
    read_budget::ReadBudget, MaterializeImplementationBindingInput,
    MaterializeVerificationBindingInput, StateStore,
};
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::VerificationMethod;
use provenance_macros::verifies;
use serde_json::{json, Value};

/// The row budget of a record, which the writer and each read apply.
const BUDGET: usize = crate::cache::read::page::RECORD_BYTES;

/// One kind: the input of a record with the id suffix `suffix` and `pad` in
/// its long field, and the writer of that input, which gives the bytes of
/// the stored row.
struct Kind {
    name: &'static str,
    input: fn(suffix: &str, pad: String) -> Value,
    write: fn(&StateStore, Value) -> anyhow::Result<usize>,
    refused_id: fn(&str) -> bool,
}

fn row<T: ReadBudget>(record: anyhow::Result<T>) -> anyhow::Result<usize> {
    record?.read_bytes()
}

fn parse<T: serde::de::DeserializeOwned>(input: Value) -> T {
    serde_json::from_value(input).unwrap()
}

fn kinds() -> Vec<Kind> {
    vec![
        Kind {
            name: "source",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("source_{suffix}"), "name": pad,
                    "source_type": "policy", "supersedes": []})
            },
            write: |store, input| row(store.create_source(parse(input))),
            refused_id: |id| id == "source_overb",
        },
        Kind {
            name: "requirement",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("req_{suffix}"),
                    "statement": "The system keeps the data", "description": pad,
                    "status": "active", "refines": [], "depends_on": [], "supersedes": []})
            },
            write: |store, input| row(store.create_requirement(parse(input))),
            refused_id: |id| id == "req_overb",
        },
        Kind {
            name: "rule",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("rule_{suffix}"),
                    "description": pad, "requirement_ids": ["req_overtime"],
                    "resolution_ids": [], "statement": "The system shows the data",
                    "status": "draft", "severity": "high"})
            },
            write: |store, input| row(store.create_rule(parse(input))),
            refused_id: |id| id == "rule_overb",
        },
        Kind {
            name: "topic",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("topic_{suffix}"),
                    "requirement_id": "req_overtime", "title": pad, "status": "open",
                    "links": []})
            },
            write: |store, input| row(store.create_topic(parse(input))),
            refused_id: |id| id == "topic_overb",
        },
        Kind {
            name: "question",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("question_{suffix}"),
                    "topic_id": "topic_probe", "question": pad, "resolution_method": "ask",
                    "depends_on": [], "status": "open", "links": []})
            },
            write: |store, input| row(store.create_question(parse(input))),
            refused_id: |id| id == "question_overb",
        },
        Kind {
            name: "resolution",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("res_{suffix}"), "title": pad,
                    "requirement_ids": ["req_overtime"], "supersedes": [],
                    "position": "Keep the data", "rationale": "The data is needed",
                    "status": "draft", "inputs": []})
            },
            write: |store, input| row(store.create_resolution(parse(input))),
            refused_id: |id| id == "res_overb",
        },
        Kind {
            name: "contribution",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("contrib_{suffix}"),
                    "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
                    "participant_slot": "reviewer", "stance": "support",
                    "strongest_finding": pad, "evidence_references": [],
                    "material_claims": [], "risks": [], "objections": [], "challenges": [],
                    "suggested_artifact_changes": [], "unsupported_recommendations": [],
                    "uncertainty": {"level": "low", "rationale": "Direct evidence"},
                    "open_questions": []})
            },
            write: |store, input| row(store.create_contribution(parse(input))),
            refused_id: |id| id == "contrib_overb",
        },
        Kind {
            name: "synthesis_packet",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("synthesis_{suffix}"),
                    "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
                    "contribution_ids": ["contrib_probe"], "summary": pad,
                    "claims": [], "disagreements": [], "unresolved_questions": []})
            },
            write: |store, input| row(store.create_synthesis_packet(parse(input))),
            refused_id: |id| id == "synthesis_overb",
        },
        Kind {
            name: "proposal",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("proposal_{suffix}"),
                    "proposal_key": format!("proposal-{suffix}"),
                    "proposal_type": "requirement_candidate", "title": "A proposal",
                    "summary": pad,
                    "traceability": {"target": {"artifact_type": "requirement",
                        "artifact_id": "req_overtime"}, "source_ids": [],
                        "evidence_references": [], "supporting_claim_ids": []},
                    "builds_on": [], "promotion_state": "proposed"})
            },
            write: |store, input| row(store.create_proposal_card(parse(input))),
            refused_id: |id| id == "proposal_overb",
        },
        Kind {
            name: "disposition",
            input: |suffix, pad| {
                json!({"scope_id": "default", "id": format!("disposition_{suffix}"),
                    "proposal_id": "proposal_probe", "decision": "rejected",
                    "rationale": pad, "actor": {"identity_type": "human", "id": "ben"}})
            },
            write: |store, input| row(store.create_disposition(parse(input))),
            refused_id: |id| id == "disposition_overb",
        },
        Kind {
            name: "implementation_binding",
            input: |suffix, pad| {
                json!({"scope_id": "default", "rule_id": format!("bind_{suffix}"),
                    "declared_by": "spec.md", "file": "check.rs",
                    "symbol": format!("check{pad}")})
            },
            write: |store, input| {
                row(store.materialize_implementation_binding(
                    MaterializeImplementationBindingInput {
                        scope_id: parse(input["scope_id"].clone()),
                        rule_id: parse(input["rule_id"].clone()),
                        declared_by: parse(input["declared_by"].clone()),
                        file: parse(input["file"].clone()),
                        symbol: parse(input["symbol"].clone()),
                    },
                ))
            },
            refused_id: |id| id.starts_with("implementation_binding_"),
        },
        Kind {
            name: "verification_binding",
            input: |suffix, pad| {
                json!({"scope_id": "default", "rule_id": format!("bind_{suffix}"),
                    "key": "check", "declared_by": "spec.md",
                    "file": "check.rs", "symbol": format!("check{pad}")})
            },
            write: |store, input| {
                row(
                    store.materialize_verification_binding(MaterializeVerificationBindingInput {
                        scope_id: parse(input["scope_id"].clone()),
                        rule_id: parse(input["rule_id"].clone()),
                        key: parse(input["key"].clone()),
                        method: VerificationMethod::Examples,
                        declared_by: parse(input["declared_by"].clone()),
                        file: parse(input["file"].clone()),
                        symbol: parse(input["symbol"].clone()),
                    }),
                )
            },
            refused_id: |id| id.starts_with("verification_binding_"),
        },
    ]
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn each_kind_lands_at_the_record_budget_and_is_refused_one_byte_over() {
    let (dir, store, _scope) = seeded_requirement_store();
    std::fs::write(dir.path().join("check.rs"), "fn check() {}\n").unwrap();
    for suffix in ["probe", "atbud", "overb"] {
        let mut rule = (kinds()[2].input)(suffix, "p".into());
        rule["id"] = json!(format!("bind_{suffix}"));
        store.create_rule(parse(rule)).unwrap();
    }
    let budget = u64::try_from(BUDGET).unwrap();
    for kind in kinds() {
        let probe = (kind.write)(&store, (kind.input)("probe", "p".into()))
            .unwrap_or_else(|error| panic!("{}: {error:#}", kind.name));
        let base = probe - 1;
        let at = (kind.write)(&store, (kind.input)("atbud", "p".repeat(BUDGET - base)));
        let over = (kind.write)(&store, (kind.input)("overb", "p".repeat(BUDGET + 1 - base)));

        assert_eq!(
            at.unwrap_or_else(|error| panic!("{}: {error:#}", kind.name)),
            BUDGET
        );
        let error = over.expect_err(kind.name);
        let failure = &error
            .downcast_ref::<SourceFailure>()
            .expect(kind.name)
            .failure;
        let WriteFailure::RecordTooLarge {
            record_id: Some(id),
            bytes,
            limit,
            ..
        } = failure
        else {
            panic!("{}: {failure:?}", kind.name);
        };
        assert!((kind.refused_id)(id), "{}: {id}", kind.name);
        assert_eq!((*bytes, *limit), (budget + 1, budget), "{}", kind.name);
    }
}
