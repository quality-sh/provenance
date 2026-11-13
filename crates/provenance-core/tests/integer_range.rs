//! Each integer field of a committed record that the read database holds
//! refuses a value past ±(2^53 − 1) when the record is parsed, and takes
//! each value in that range. An optional time field reads `null` and an
//! absent field as no value.
//!
//! No Rule: an implementation aid. The range is a limit of the read
//! database port, and no Rule of the graph states the parse refusal.

use provenance_core::threads::Discussion;
use provenance_core::{Message, Question, RequirementReview, Resolution, Thread, Topic};
use serde_json::{json, Value};

const LARGEST: i64 = (1 << 53) - 1;

/// Parses `record` as a `T` and gives it back as JSON.
fn parse<T: serde::de::DeserializeOwned + serde::Serialize>(
    record: &Value,
) -> Result<Value, String> {
    serde_json::from_value::<T>(record.clone())
        .map(|parsed| serde_json::to_value(parsed).unwrap())
        .map_err(|error| error.to_string())
}

type Parse = fn(&Value) -> Result<Value, String>;

/// One integer field: the record type, a valid record, the field, and
/// whether the field is optional.
struct Field {
    name: &'static str,
    parse: Parse,
    record: Value,
    field: &'static str,
    optional: bool,
    /// The smallest value that the field takes.
    least: i64,
}

fn fields() -> Vec<Field> {
    let resolution = json!({"scope_id":"default","id":"res_a","title":"Decision",
        "position":"Use the threshold.","rationale":"The clause sets it.","status":"approved",
        "inputs":[],"requirement_ids":["req_a"]});
    let topic = json!({"scope_id":"default","id":"topic_a","requirement_id":"req_a",
        "title":"Root","status":"open","links":[]});
    let question = json!({"scope_id":"default","id":"question_a","topic_id":"topic_a",
        "requirement_id":"req_a","question":"Which root?","resolution_method":"ask",
        "status":"open","links":[]});
    let review = json!({"scope_id":"default","id":"requirement_review_a","rule_id":"rule_a",
        "requirement_id":"req_a","field":"statement","before":"Old.","after":"New.",
        "changed_at":1});
    let thread = json!({"scope_id":"default","id":"thread_a",
        "parent":{"node_type":"rule","node_id":"rule_a"},"status":"active","created_at":1});
    let message = json!({"scope_id":"default","id":"msg_a","thread_id":"thread_a",
        "role":"user","body":"Yes.","created_at":1});
    let discussion = json!({"scope_id":"default","discussion_id":"discussion_a",
        "parent":{"node_type":"topic","node_id":"topic_a"},"thread_id":"thread_a",
        "root_message_id":"msg_a","message_ids":["msg_a"],"status":"active","version":1,
        "actor":"cli","outcomes":[],"disposition_id":null});
    let field = |name, parse: Parse, record: &Value, field, optional, least| Field {
        name,
        parse,
        record: record.clone(),
        field,
        optional,
        least,
    };
    vec![
        field(
            "Resolution",
            parse::<Resolution>,
            &resolution,
            "approved_at",
            true,
            -LARGEST,
        ),
        field(
            "Topic",
            parse::<Topic>,
            &topic,
            "claimed_at",
            true,
            -LARGEST,
        ),
        field(
            "Question",
            parse::<Question>,
            &question,
            "claimed_at",
            true,
            -LARGEST,
        ),
        field(
            "RequirementReview",
            parse::<RequirementReview>,
            &review,
            "changed_at",
            false,
            -LARGEST,
        ),
        field(
            "RequirementReview",
            parse::<RequirementReview>,
            &review,
            "cleared_at",
            true,
            -LARGEST,
        ),
        field(
            "Thread",
            parse::<Thread>,
            &thread,
            "created_at",
            false,
            -LARGEST,
        ),
        field(
            "Message",
            parse::<Message>,
            &message,
            "created_at",
            false,
            -LARGEST,
        ),
        field(
            "Discussion",
            parse::<Discussion>,
            &discussion,
            "version",
            false,
            1,
        ),
    ]
}

/// What `field` reads for `value`: the parsed value, or the refusal.
fn read(field: &Field, value: Value) -> Result<Value, String> {
    let mut record = field.record.clone();
    record[field.field] = value;
    (field.parse)(&record).map(|parsed| parsed[field.field].clone())
}

/// Each field that does not answer as the range says, with the value and
/// what it read. The list names every such field at once.
#[test]
fn each_integer_field_refuses_a_value_past_the_range_when_parsed() {
    let mut wrong = Vec::new();
    for field in fields() {
        let label = format!("{}.{}", field.name, field.field);
        for past in [LARGEST + 1, -LARGEST - 1, i64::MAX, i64::MIN] {
            match read(&field, json!(past)) {
                Err(error) if error.contains(&format!("integer `{past}`")) => {}
                other => wrong.push(format!("{label} = {past}: {other:?}")),
            }
        }
        for end in [LARGEST, field.least] {
            let other = read(&field, json!(end));
            if other != Ok(json!(end)) {
                wrong.push(format!("{label} = {end}: {other:?}"));
            }
        }
        if field.optional {
            let null = read(&field, Value::Null);
            let mut absent = field.record.clone();
            absent.as_object_mut().unwrap().remove(field.field);
            let absent = (field.parse)(&absent).map(|parsed| parsed.get(field.field).cloned());
            if null != Ok(Value::Null) || absent != Ok(None) {
                wrong.push(format!("{label} null: {null:?}, absent: {absent:?}"));
            }
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
}

/// The refusal names the range as the expected value.
#[test]
fn the_refusal_names_the_range() {
    let field = &fields()[6];
    assert_eq!(
        read(field, json!(LARGEST + 1)),
        Err(
            "invalid value: integer `9007199254740992`, expected an integer from \
             -9007199254740991 to 9007199254740991"
                .to_owned()
        )
    );
}
