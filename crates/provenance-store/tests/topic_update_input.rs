//! Implementation aid: these tests pin how a Topic update reads its list of
//! fields to clear, so that a change of the type of that list keeps the
//! results and the error texts. No Rule states this behaviour.
use provenance_store::state_store::UpdateTopicInput;
use serde_json::{json, Value};

fn read(clear: Option<Value>) -> Result<usize, String> {
    let mut input = json!({"scope_id": "default", "id": "topic-a", "title": "A"});
    if let Some(clear) = clear {
        input["clear_fields"] = clear;
    }
    serde_json::from_value::<UpdateTopicInput>(input)
        .map(|input| input.clear_fields.len())
        .map_err(|error| error.to_string())
}

fn row(list: Option<&str>, reply: Result<usize, &str>) -> (Option<String>, Result<usize, String>) {
    (list.map(str::to_owned), reply.map_err(str::to_owned))
}

#[test]
fn a_topic_update_reads_only_an_empty_list_of_fields_to_clear() {
    let lists = [
        None,
        Some(json!([])),
        Some(json!(["title"])),
        Some(json!(["status", "title"])),
        Some(json!([{"title": null}])),
        Some(json!([1])),
        Some(json!([null])),
        Some(json!("title")),
        Some(json!(true)),
    ];
    let actual: Vec<_> = lists
        .into_iter()
        .map(|list| (list.as_ref().map(Value::to_string), read(list)))
        .collect();
    let expected = [
        row(None, Ok(0)),
        row(Some("[]"), Ok(0)),
        row(
            Some(r#"["title"]"#),
            Err("unknown variant `title`, there are no variants"),
        ),
        row(
            Some(r#"["status","title"]"#),
            Err("unknown variant `status`, there are no variants"),
        ),
        row(
            Some(r#"[{"title":null}]"#),
            Err("unknown variant `title`, there are no variants"),
        ),
        row(
            Some("[1]"),
            Err("invalid type: integer `1`, expected string or map"),
        ),
        row(
            Some("[null]"),
            Err("invalid type: null, expected string or map"),
        ),
        row(
            Some(r#""title""#),
            Err(r#"invalid type: string "title", expected a sequence"#),
        ),
        row(
            Some("true"),
            Err("invalid type: boolean `true`, expected a sequence"),
        ),
    ];
    assert_eq!(actual, expected);
}

#[cfg(feature = "schema")]
#[test]
fn the_topic_update_schema_takes_an_empty_list_of_fields_to_clear() {
    let schema = serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = schemars::generate::Contract::Deserialize)
            .into_generator()
            .into_root_schema_for::<UpdateTopicInput>(),
    )
    .unwrap();
    assert_eq!(
        schema["properties"]["clear_fields"],
        json!({"type": "array", "items": {"$ref": "#/$defs/TopicClearField"}})
    );
    assert_eq!(schema["$defs"]["TopicClearField"], json!(false));
}
