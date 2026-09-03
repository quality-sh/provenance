//! Rows 35 to 39: the typed bindings, reviews, ownership, and the rule's
//! text citation.

use super::model::{Act, Clear, Read, Row};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};

fn spec(statement: &str, implemented: bool, with_requirement: bool) -> Value {
    let mut rule = json!({
        "key": "expiry",
        "requirements": ["sharing"],
        "statement": "Share links expire within 30 days"
    });
    if implemented {
        rule["implementation"] = json!({"file": "share-links.ts", "symbol": "createShareLink"});
    }
    let mut document = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "map",
        "declared_by": "spec://map",
        "sources": [{"key": "retention", "name": "Retention policy", "kind": "policy"}],
        "requirements": [],
        "rules": []
    });
    if with_requirement {
        document["requirements"] =
            json!([{"key": "sharing", "statement": statement, "sources": ["retention"]}]);
        document["rules"] = json!([rule]);
    }
    document
}

fn verification() -> Value {
    json!({
        "rule": "{spec_rule}",
        "key": "share-link-expiry",
        "method": "examples",
        "declared_by": "ci://map",
        "file": "share-links.test.ts",
        "symbol": "expiry test"
    })
}

pub fn sdk_rows() -> Vec<Row> {
    vec![
        Row {
            number: 35,
            relation: "rule_id",
            owner: "implementation binding",
            act: Act::Sdk("apply", spec("Shares are time bounded", true, true)),
            read: Read::Output {
                pointer: "/implementation_bindings/0/rule_id",
                expect: json!("{spec_rule}"),
                capture: &[
                    ("spec_rule", "/resources/2/id"),
                    ("spec_req", "/resources/1/id"),
                ],
            },
            clear: Clear::Sdk(
                "apply",
                spec("Shares are time bounded", false, true),
                json!(null),
            ),
        },
        Row {
            number: 36,
            relation: "rule_id",
            owner: "verification binding",
            act: Act::Sdk("begin-verification", verification()),
            read: Read::Output {
                pointer: "/rule_id",
                expect: json!("{spec_rule}"),
                capture: &[("run", "/id")],
            },
            clear: Clear::None,
        },
        Row {
            number: 37,
            relation: "requirement_id",
            owner: "requirement review",
            act: Act::Sdk(
                "apply",
                spec("Shares are time bounded and revocable", false, true),
            ),
            read: Read::Sdk {
                operation: "evidence",
                request: json!({"rule": "{spec_rule}"}),
                pointer: "/reviews/0/requirement_id",
                expect: json!("{spec_req}"),
            },
            clear: Clear::Sdk("begin-verification", verification(), json!(null)),
        },
        Row {
            number: 38,
            relation: "declared_by",
            owner: "requirement",
            act: Act::Derived("sdk apply in row 35 declared the requirement"),
            read: Read::Record {
                kind: "requirement",
                id: "{spec_req}",
                pointer: "/declared_by",
                expect: json!("spec://map"),
            },
            clear: Clear::Sdk("apply", spec("", false, false), json!("spec://map")),
        },
        Row {
            number: 38,
            relation: "retired",
            owner: "requirement",
            act: Act::Derived("the apply in the row above omitted the requirement"),
            read: Read::Record {
                kind: "requirement",
                id: "{spec_req}",
                pointer: "/retired",
                expect: json!(true),
            },
            clear: Clear::None,
        },
        Row {
            number: 39,
            relation: "source_document",
            owner: "rule",
            act: Act::Derived(
                "rules create --source-document, in setup; a text citation no walk reads",
            ),
            read: Read::Record {
                kind: "rule",
                id: "rule_a",
                pointer: "/source_document",
                expect: json!("docs/award.md"),
            },
            clear: Clear::None,
        },
    ]
}
