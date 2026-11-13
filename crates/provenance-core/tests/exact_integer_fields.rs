//! Each integer field of a committed record that the read database binds
//! refuses a value past the range ±(2^53 − 1) when the record is parsed,
//! and reads each value in the range unchanged.
//!
//! No Rule: an implementation aid. Each row of the table is one field, so
//! a field that loses its range check fails here by name.

use provenance_core::model::exact_integer::LARGEST_EXACT_INTEGER;
use provenance_core::threads::Discussion;
use provenance_core::{Message, Question, RequirementReview, Resolution, Thread, Topic};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

const LARGEST: i64 = LARGEST_EXACT_INTEGER;

/// The refusal text of a value past the range.
const EXPECTED: &str = "expected an integer from -9007199254740991 to 9007199254740991";

/// Parses `line` as `T` and writes it again, or gives the parse error.
type Read = fn(Value) -> Result<Value, String>;

fn read<T: DeserializeOwned + Serialize>(line: Value) -> Result<Value, String> {
    let record: T = serde_json::from_value(line).map_err(|error| error.to_string())?;
    Ok(serde_json::to_value(record).unwrap())
}

/// One field: the record that holds it, a valid line of that record, and
/// whether the field can be absent.
struct Field {
    name: &'static str,
    key: &'static str,
    line: Value,
    read: Read,
    optional: bool,
}

fn fields() -> Vec<Field> {
    let parent = json!({"node_type":"requirement","node_id":"req_a"});
    vec![
        Field {
            name: "Resolution.approved_at",
            key: "approved_at",
            line: json!({"scope_id":"default","id":"res_a","title":"Decision",
                "position":"Use it.","rationale":"It works.","status":"approved"}),
            read: read::<Resolution>,
            optional: true,
        },
        Field {
            name: "Topic.claimed_at",
            key: "claimed_at",
            line: json!({"scope_id":"default","id":"topic_a","requirement_id":"req_a",
                "title":"Topic","status":"open","claimed_by":"worker","links":[]}),
            read: read::<Topic>,
            optional: true,
        },
        Field {
            name: "Question.claimed_at",
            key: "claimed_at",
            line: json!({"scope_id":"default","id":"q_a","topic_id":"topic_a",
                "requirement_id":"req_a","question":"Which?","resolution_method":"ask",
                "status":"open","claimed_by":"worker"}),
            read: read::<Question>,
            optional: true,
        },
        Field {
            name: "RequirementReview.changed_at",
            key: "changed_at",
            line: review_line(),
            read: read::<RequirementReview>,
            optional: false,
        },
        Field {
            name: "RequirementReview.cleared_at",
            key: "cleared_at",
            line: review_line(),
            read: read::<RequirementReview>,
            optional: true,
        },
        Field {
            name: "Thread.created_at",
            key: "created_at",
            line: json!({"scope_id":"default","id":"thr_a","parent":parent,
                "status":"active","created_at":0}),
            read: read::<Thread>,
            optional: false,
        },
        Field {
            name: "Message.created_at",
            key: "created_at",
            line: json!({"scope_id":"default","id":"msg_a","thread_id":"thr_a",
                "role":"user","body":"Hello.","created_at":0}),
            read: read::<Message>,
            optional: false,
        },
    ]
}

fn review_line() -> Value {
    json!({"scope_id":"default","id":"requirement_review_a","rule_id":"rule_a",
        "requirement_id":"req_a","field":"statement","before":"Old.","after":"New.",
        "changed_at":0})
}

/// `line` with `field` set to `value`.
fn with(line: &Value, field: &str, value: Value) -> Value {
    let mut line = line.clone();
    line[field] = value;
    line
}

/// The faults of one field: a value in the range that does not read back,
/// or a value past the range that is not refused with the range text.
fn faults(field: &Field) -> Vec<String> {
    let mut faults = Vec::new();
    for value in [LARGEST, -LARGEST, 0] {
        match (field.read)(with(&field.line, field.key, json!(value))) {
            Ok(written) if written[field.key] == json!(value) => {}
            other => faults.push(format!("{value} in the range: {other:?}")),
        }
    }
    for value in [LARGEST + 1, -LARGEST - 1, i64::MAX, i64::MIN] {
        match (field.read)(with(&field.line, field.key, json!(value))) {
            Err(error) if error.contains(&format!("integer `{value}`, {EXPECTED}")) => {}
            other => faults.push(format!("{value} past the range: {other:?}")),
        }
    }
    if field.optional {
        let mut absent = field.line.clone();
        absent.as_object_mut().unwrap().remove(field.key);
        for line in [absent, with(&field.line, field.key, Value::Null)] {
            match (field.read)(line) {
                Ok(written) if written.get(field.key).is_none() => {}
                other => faults.push(format!("an absent value: {other:?}")),
            }
        }
    }
    faults
}

#[test]
fn each_committed_time_field_refuses_a_value_past_the_range() {
    let refused: Vec<String> = fields()
        .iter()
        .flat_map(|field| {
            faults(field)
                .into_iter()
                .map(move |fault| format!("{}: {fault}", field.name))
        })
        .collect();
    assert_eq!(refused, Vec::<String>::new());
}

/// The version of a Discussion is an unsigned count. It reads each count
/// up to 2^53 − 1 and refuses a larger one.
#[test]
fn a_discussion_version_past_the_range_is_refused() {
    let line = json!({"scope_id":"default","discussion_id":"discussion_a",
        "parent":{"node_type":"requirement","node_id":"req_a"},"thread_id":"thr_a",
        "root_message_id":"msg_a","message_ids":["msg_a"],"status":"active","version":1,
        "actor":"worker","outcomes":[],"disposition_id":null});
    let mut faults = Vec::new();
    for value in [1, LARGEST.unsigned_abs()] {
        match read::<Discussion>(with(&line, "version", json!(value))) {
            Ok(written) if written["version"] == json!(value) => {}
            other => faults.push(format!("{value} in the range: {other:?}")),
        }
    }
    for value in [
        LARGEST.unsigned_abs() + 1,
        i64::MAX.unsigned_abs(),
        u64::MAX,
    ] {
        match read::<Discussion>(with(&line, "version", json!(value))) {
            Err(error) if error.contains(&format!("integer `{value}`, {EXPECTED}")) => {}
            other => faults.push(format!("{value} past the range: {other:?}")),
        }
    }
    assert_eq!(faults, Vec::<String>::new());
}
