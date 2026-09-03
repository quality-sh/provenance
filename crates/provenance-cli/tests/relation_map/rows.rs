//! Rows 1 to 24 of the relation map (gist 04, "relation ontology by
//! authoring act"): the graph relations, the shaping records, and the
//! collaboration records, each as the act that writes it after the cut.

use super::model::{Act, Clear, Read, Row};
use serde_json::json;

const fn record(
    kind: &'static str,
    id: &'static str,
    pointer: &'static str,
    expect: serde_json::Value,
) -> Read {
    Read::Record {
        kind,
        id,
        pointer,
        expect,
    }
}

const fn row(
    number: u8,
    relation: &'static str,
    owner: &'static str,
    act: Act,
    read: Read,
    clear: Clear,
) -> Row {
    Row {
        number,
        relation,
        owner,
        act,
        read,
        clear,
    }
}

/// Rows 1 to 9: the relations that were edges.
#[rustfmt::skip]
pub fn graph_rows() -> Vec<Row> {
    vec![
        row(1, "cites", "requirement",
            Act::Cli(&["requirements", "source-ref", "add", "--requirement-id", "req_a", "--source-id", "source_award", "--clause", "4.2"]),
            record("requirement", "req_a", "/source_refs", json!([{"source_id": "source_award", "clause": "4.2"}])),
            Clear::Cli(&["requirements", "source-ref", "clear", "--requirement-id", "req_a", "--source-id", "source_award"], json!(null))),
        row(2, "refines", "requirement",
            Act::Cli(&["requirements", "refines", "set", "--requirement-id", "req_b", "--target-id", "req_a"]),
            record("requirement", "req_b", "/refines", json!("req_a")),
            Clear::Cli(&["requirements", "refines", "clear", "--requirement-id", "req_b"], json!(null))),
        row(3, "depends_on", "requirement",
            Act::Cli(&["requirements", "depends-on", "add", "--requirement-id", "req_b", "--target-id", "req_c"]),
            record("requirement", "req_b", "/depends_on", json!(["req_c"])),
            Clear::Cli(&["requirements", "depends-on", "clear", "--requirement-id", "req_b", "--target-id", "req_c"], json!(null))),
        row(4, "contradicts", "question",
            Act::Cli(&["questions", "contradicts", "set", "--id", "question_q", "--target-id", "req_b"]),
            record("question", "question_q", "/contradicts", json!("req_b")),
            Clear::Cli(&["questions", "contradicts", "clear", "--id", "question_q"], json!(null))),
        row(5, "supersedes", "requirement",
            Act::Cli(&["requirements", "supersedes", "add", "--requirement-id", "req_c", "--target-id", "req_b"]),
            record("requirement", "req_c", "/supersedes", json!(["req_b"])),
            Clear::Cli(&["requirements", "supersedes", "clear", "--requirement-id", "req_c", "--target-id", "req_b"], json!(null))),
        row(6, "requirement_ids", "resolution",
            Act::Derived("needs is dropped; the resolution's requirement_ids (row 7) carries the fact"),
            record("resolution", "res_a", "/requirement_ids", json!(["req_a"])),
            Clear::None),
        row(7, "requirement_ids", "resolution",
            Act::Cli(&["resolutions", "requirement", "add", "--resolution-id", "res_a", "--target-id", "req_b"]),
            record("resolution", "res_a", "/requirement_ids", json!(["req_a", "req_b"])),
            Clear::Cli(&["resolutions", "requirement", "clear", "--resolution-id", "res_a", "--target-id", "req_b"], json!(["req_a"]))),
        row(8, "spawned_by", "requirement",
            Act::Cli(&["requirements", "spawned-by", "set", "--requirement-id", "req_c", "--target-id", "res_a"]),
            record("requirement", "req_c", "/spawned_by", json!("res_a")),
            Clear::Cli(&["requirements", "spawned-by", "clear", "--requirement-id", "req_c"], json!(null))),
        row(9, "requirement_ids", "rule",
            Act::Cli(&["rules", "requirement", "add", "--rule-id", "rule_a", "--target-id", "req_b"]),
            record("rule", "rule_a", "/requirement_ids", json!(["req_a", "req_b"])),
            Clear::Cli(&["rules", "requirement", "clear", "--rule-id", "rule_a", "--target-id", "req_b"], json!(["req_a"]))),
        row(9, "resolution_ids", "rule",
            Act::Cli(&["rules", "resolution", "add", "--rule-id", "rule_a", "--target-id", "res_a"]),
            record("rule", "rule_a", "/resolution_ids", json!(["res_a"])),
            Clear::Cli(&["rules", "resolution", "clear", "--rule-id", "rule_a", "--target-id", "res_a"], json!(null))),
    ]
}

/// Rows 10 to 21: the reference fields the shaping records carry.
#[rustfmt::skip]
pub fn shaping_rows() -> Vec<Row> {
    vec![
        row(10, "requirement_id", "boundary",
            Act::Derived("boundaries create --requirement-id, in setup"),
            record("boundary", "boundary_b", "/requirement_id", json!("req_a")), Clear::Immutable),
        row(11, "requirement_id", "topic",
            Act::Derived("topics create --requirement-id, in setup"),
            record("topic", "topic_t", "/requirement_id", json!("req_a")), Clear::Immutable),
        row(12, "topic_id", "question",
            Act::Derived("questions create --topic-id, in setup"),
            record("question", "question_q", "/topic_id", json!("topic_t")), Clear::Immutable),
        row(13, "requirement_id", "question",
            Act::Derived("copied from the topic when the question is created"),
            record("question", "question_q", "/requirement_id", json!("req_a")), Clear::Immutable),
        row(14, "resolution_id", "question",
            Act::Cli(&["questions", "update", "--id", "question_q", "--resolution-id", "res_a"]),
            record("question", "question_q", "/resolution_id", json!("res_a")), Clear::None),
        row(15, "domain_id", "requirement",
            Act::Derived("requirements create --domain-id, in setup"),
            record("requirement", "req_a", "/domain_id", json!("domain_payroll")), Clear::None),
        row(16, "cites", "requirement",
            Act::Cli(&["requirements", "source-ref", "add", "--requirement-id", "req_a", "--source-id", "source_award_2019"]),
            record("requirement", "req_a", "/source_refs/0/source_id", json!("source_award_2019")), Clear::None),
        row(17, "links", "topic",
            Act::Derived("topics create --links-json, in setup"),
            record("topic", "topic_t", "/links/0/target_id", json!("rule_a")), Clear::None),
        row(18, "links", "question",
            Act::Cli(&["questions", "update", "--id", "question_q", "--links-json", r#"[{"target_type":"rule","target_id":"rule_a"}]"#]),
            record("question", "question_q", "/links/0/target_id", json!("rule_a")),
            Clear::Cli(&["questions", "update", "--id", "question_q", "--links-json", "[]"], json!(null))),
        row(19, "supersedes", "source",
            Act::Cli(&["sources", "supersedes", "add", "--source-id", "source_award", "--target-id", "source_award_2019"]),
            record("source", "source_award", "/supersedes", json!(["source_award_2019"])),
            Clear::Cli(&["sources", "supersedes", "clear", "--source-id", "source_award", "--target-id", "source_award_2019"], json!(null))),
        row(20, "supersedes", "resolution",
            Act::Cli(&["resolutions", "supersedes", "add", "--resolution-id", "res_b", "--target-id", "res_a"]),
            record("resolution", "res_b", "/supersedes", json!(["res_a"])),
            Clear::Cli(&["resolutions", "supersedes", "clear", "--resolution-id", "res_b", "--target-id", "res_a"], json!(null))),
        row(21, "cites", "boundary",
            Act::Derived("boundaries create --source-id, in setup"),
            record("boundary", "boundary_b", "/source_ref/source_id", json!("source_award")), Clear::Immutable),
    ]
}

/// Rows 22 to 24: threads, messages, and origin pointers.
#[rustfmt::skip]
pub fn collaboration_rows() -> Vec<Row> {
    vec![
        row(22, "thread parent", "thread",
            Act::Cli(&["thread", "post", "--parent-type", "requirement", "--parent-id", "req_a", "--role", "user", "A note"]),
            Read::Output { pointer: "/thread/parent/node_id", expect: json!("req_a"), capture: &[("thread", "/thread/id")] },
            Clear::Immutable),
        row(23, "thread_id", "message",
            Act::Derived("the same post mints the message in the thread"),
            Read::OutputOf { row: 22, pointer: "/message/thread_id", expect: json!("{thread}") },
            Clear::Immutable),
        row(24, "origin_thread", "requirement",
            Act::Cli(&["requirements", "create", "--id", "req_origin", "--statement", "The origin is kept", "--origin-thread", "{thread}"]),
            record("requirement", "req_origin", "/origin_thread", json!("{thread}")),
            Clear::Immutable),
    ]
}
