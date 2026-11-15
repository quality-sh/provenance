//! Implementation aids: the check of each keyword that the shape check implements, and a walk
//! of the ten success schemas that fails when a schema holds a keyword or a pattern that no case
//! here checks. No Rule binds the check.
//!
//! Each case gives a schema, values that hold it and values that break it. A value that breaks
//! a case breaks only the keyword of that case, so a check of that keyword that always passes
//! fails the case.

use super::{Shape, DESCRIBING, PATTERNS, TESTED};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// A schema, values that it accepts and values that it refuses.
struct Case {
    keyword: &'static str,
    schema: Value,
    holds: Vec<Value>,
    breaks: Vec<Value>,
}

fn case(keyword: &'static str, schema: Value, holds: Vec<Value>, breaks: Vec<Value>) -> Case {
    Case {
        keyword,
        schema,
        holds,
        breaks,
    }
}

const SCOPE: &str = "^[a-z0-9_-]+$";
const COMMIT: &str = "^([0-9a-f]{40}|[0-9a-f]{64})$";
const TEXT: &str = "\\S";

/// The cases of each keyword. A keyword can have more than one case.
fn cases() -> Vec<Case> {
    [
        value_cases(),
        member_cases(),
        branch_cases(),
        bound_cases(),
        pattern_cases(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The cases of the keywords on one value: `$ref`, `type`, `enum` and `const`.
fn value_cases() -> Vec<Case> {
    vec![
        case(
            "$ref",
            json!({"$defs": {"name": {"type": "string"}}, "$ref": "#/$defs/name"}),
            vec![json!("a")],
            vec![json!(1)],
        ),
        case(
            "type",
            json!({"type": "string"}),
            vec![json!("a")],
            vec![json!(1), json!(null), json!({}), json!([])],
        ),
        case(
            "type",
            json!({"type": ["integer", "null"]}),
            vec![json!(1), json!(null)],
            vec![json!(1.5), json!("1"), json!(true)],
        ),
        case(
            "enum",
            json!({"enum": ["catch_up", "refuse_stale"]}),
            vec![json!("refuse_stale")],
            vec![json!("made_up"), json!(null)],
        ),
        case(
            "const",
            json!({"const": "requirement"}),
            vec![json!("requirement")],
            vec![json!("rule")],
        ),
    ]
}

/// The cases of the keywords on the members and items of a value.
fn member_cases() -> Vec<Case> {
    let integer = json!({"type": "integer"});
    vec![
        case(
            "properties",
            json!({"properties": {"n": integer}}),
            vec![json!({"n": 1}), json!({})],
            vec![json!({"n": "1"})],
        ),
        case(
            "required",
            json!({"required": ["n"]}),
            vec![json!({"n": 1})],
            vec![json!({}), json!({"m": 1})],
        ),
        case(
            "additionalProperties",
            json!({"additionalProperties": integer}),
            vec![json!({"a": 1, "b": 2})],
            vec![json!({"a": 1, "b": "2"})],
        ),
        case(
            "items",
            json!({"items": integer}),
            vec![json!([]), json!([1, 2])],
            vec![json!([1, "2"])],
        ),
    ]
}

/// The cases of `allOf`, `anyOf` and `oneOf`.
fn branch_cases() -> Vec<Case> {
    let integer = json!({"type": "integer"});
    vec![
        case(
            "allOf",
            json!({"allOf": [integer, {"minimum": 0}]}),
            vec![json!(1)],
            vec![json!("1"), json!(-1)],
        ),
        case(
            "anyOf",
            json!({"anyOf": [integer, {"type": "null"}]}),
            vec![json!(1), json!(null)],
            vec![json!("1")],
        ),
        // A value that matches both branches, or neither, breaks the count of `oneOf`.
        case(
            "oneOf",
            json!({"oneOf": [integer, {"minimum": 0}]}),
            vec![json!(-1), json!(0.5)],
            vec![json!(1), json!(-0.5)],
        ),
    ]
}

/// The cases of the six bounds.
fn bound_cases() -> Vec<Case> {
    vec![
        case(
            "minimum",
            json!({"minimum": 2}),
            vec![json!(2)],
            vec![json!(1)],
        ),
        case(
            "maximum",
            json!({"maximum": 2}),
            vec![json!(2)],
            vec![json!(3)],
        ),
        case(
            "minItems",
            json!({"minItems": 1}),
            vec![json!([1])],
            vec![json!([])],
        ),
        case(
            "maxItems",
            json!({"maxItems": 1}),
            vec![json!([1])],
            vec![json!([1, 2])],
        ),
        case(
            "minLength",
            json!({"minLength": 2}),
            vec![json!("ab"), json!("éé")],
            vec![json!("a"), json!("")],
        ),
        case(
            "maxLength",
            json!({"maxLength": 2}),
            vec![json!("ab"), json!("éé")],
            vec![json!("abc")],
        ),
    ]
}

/// A case of each known pattern.
fn pattern_cases() -> Vec<Case> {
    vec![
        case(
            "pattern",
            json!({"pattern": SCOPE}),
            vec![json!("default"), json!("req_a-1")],
            vec![json!("Not A Scope!"), json!(""), json!("a.b")],
        ),
        case(
            "pattern",
            json!({"pattern": COMMIT}),
            vec![json!("a".repeat(40)), json!("0".repeat(64))],
            vec![
                json!("a".repeat(41)),
                json!("A".repeat(40)),
                json!("g".repeat(40)),
                json!(""),
            ],
        ),
        case(
            "pattern",
            json!({"pattern": TEXT}),
            vec![json!(" a "), json!("a")],
            vec![json!(" \t\n"), json!("")],
        ),
    ]
}

/// Checks each case of `keyword`.
fn check(keyword: &str) {
    let cases: Vec<Case> = cases()
        .into_iter()
        .filter(|case| case.keyword == keyword)
        .collect();
    assert!(!cases.is_empty(), "no case checks `{keyword}`");
    for case in cases {
        let shape = Shape::new(case.schema.clone()).unwrap();
        for value in &case.holds {
            assert!(shape.accepts(value), "{} refuses {value}", case.schema);
        }
        for value in &case.breaks {
            assert!(!shape.accepts(value), "{} accepts {value}", case.schema);
        }
    }
}

macro_rules! keyword_tests {
    ($($test:ident => $keyword:literal),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                check($keyword);
            }
        )*
    };
}

keyword_tests!(
    a_ref_checks_its_definition => "$ref",
    a_type_checks_the_json_type => "type",
    an_enum_checks_its_values => "enum",
    a_const_checks_its_value => "const",
    properties_check_each_member => "properties",
    required_checks_each_member_is_present => "required",
    additional_properties_check_each_other_member => "additionalProperties",
    items_check_each_item => "items",
    all_of_checks_each_branch => "allOf",
    any_of_checks_one_branch_matches => "anyOf",
    one_of_checks_exactly_one_branch_matches => "oneOf",
    a_minimum_checks_the_number => "minimum",
    a_maximum_checks_the_number => "maximum",
    min_items_checks_the_length => "minItems",
    max_items_checks_the_length => "maxItems",
    min_length_checks_the_characters => "minLength",
    max_length_checks_the_characters => "maxLength",
    a_pattern_checks_each_known_pattern => "pattern",
);

/// Puts each keyword and each pattern of `schema` and of its subschemas in `keywords` and
/// `patterns`.
fn walk(schema: &Value, keywords: &mut BTreeSet<String>, patterns: &mut BTreeSet<String>) {
    let Value::Object(schema) = schema else {
        return;
    };
    for (keyword, rule) in schema {
        keywords.insert(keyword.clone());
        match (keyword.as_str(), rule) {
            ("pattern", Value::String(pattern)) => {
                patterns.insert(pattern.clone());
            }
            ("properties" | "$defs", Value::Object(members)) => {
                for member in members.values() {
                    walk(member, keywords, patterns);
                }
            }
            ("allOf" | "anyOf" | "oneOf", Value::Array(branches)) => {
                for branch in branches {
                    walk(branch, keywords, patterns);
                }
            }
            ("items" | "additionalProperties", rule) => walk(rule, keywords, patterns),
            _ => {}
        }
    }
}

/// Each keyword of the ten success schemas only describes a value or has a case here, and each
/// pattern has a case. Each keyword that the check implements has a case too.
#[test]
fn each_keyword_of_the_success_schemas_has_a_case() {
    let (mut keywords, mut patterns) = (BTreeSet::new(), BTreeSet::new());
    for checks in crate::port::CHECKS.iter() {
        walk(&checks.success.root, &mut keywords, &mut patterns);
    }
    let all = cases();
    let checked: BTreeSet<&str> = all.iter().map(|case| case.keyword).collect();
    let unchecked: Vec<&String> = keywords
        .iter()
        .filter(|keyword| {
            !checked.contains(keyword.as_str())
                && !DESCRIBING.contains(&keyword.as_str())
                && !keyword.starts_with("x-")
        })
        .collect();
    assert_eq!(unchecked, Vec::<&String>::new(), "keywords with no case");

    let cased: BTreeSet<&str> = all
        .iter()
        .filter_map(|case| case.schema.get("pattern").and_then(Value::as_str))
        .collect();
    let uncased: Vec<&String> = patterns
        .iter()
        .filter(|pattern| !cased.contains(pattern.as_str()))
        .collect();
    assert_eq!(uncased, Vec::<&String>::new(), "patterns with no case");

    let implemented: BTreeSet<&str> = TESTED.into_iter().collect();
    assert_eq!(checked, implemented);
    let known: BTreeSet<&str> = PATTERNS.iter().map(|(pattern, _)| *pattern).collect();
    assert_eq!(cased, known);
}
