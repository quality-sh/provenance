use super::support::{raw_rows, Fixture};
use serde_json::{json, Value};

fn rows_except(rows: &[String], id: &str) -> Vec<String> {
    rows.iter()
        .filter(|row| serde_json::from_str::<Value>(row).unwrap()["id"] != id)
        .cloned()
        .collect()
}

fn assert_kept(after: &[String], untouched: &[String]) {
    for row in untouched {
        assert_eq!(
            after.iter().filter(|saved| *saved == row).count(),
            1,
            "untouched row must survive byte for byte: {row}"
        );
    }
}

// The binding to rule_porcelain_update_preserves_omissions covers only the
// edited field: the owner's description changes and the other fields of that
// requirement stay. The other checks are guards with no rule of their own:
// the spec's other declarations stay unchanged, every status stays active,
// and a second apply keeps the owner's edit. No Rule states these yet, so
// they must not be read as evidence for the bound rule.
#[tokio::test]
#[provenance_macros::verifies("rule_porcelain_update_preserves_omissions", examples)]
async fn owner_edit_of_one_declaration_keeps_the_other_declarations() {
    let fixture = Fixture::new();
    let spec = json!({"schema_version":2,"spec":"fixture","declared_by":"spec://fixture",
        "requirements":[
            {"key":"save","statement":"The system saves the record."},
            {"key":"read","statement":"The system reads the record."}],
        "rules":[{"key":"save","requirement":"save","statement":"The system saves the record."}]});
    fixture.call("apply", spec.clone()).await.unwrap();
    let requirements = fixture.store.list_requirements(&fixture.scope).unwrap();
    let rules = fixture.store.list_rules(&fixture.scope).unwrap();
    assert_eq!((requirements.len(), rules.len()), (2, 1));
    let edited = requirements
        .iter()
        .find(|requirement| requirement.statement == "The system saves the record.")
        .map(|requirement| serde_json::to_value(requirement).unwrap())
        .unwrap();
    let id = edited["id"].as_str().unwrap().to_owned();
    let other_requirements = rows_except(&raw_rows(&fixture, "requirements", "req.jsonl"), &id);
    let rule_rows = raw_rows(&fixture, "rules", "rule.jsonl");
    assert_eq!((other_requirements.len(), rule_rows.len()), (1, 1));

    let updated = fixture
        .call(
            "update-requirement-native",
            json!({"scope_id":"default","id":id,"declared_by":"spec://fixture","description":"Changed"}),
        )
        .await
        .unwrap();

    let mut expected = edited;
    expected["description"] = json!("Changed");
    assert_eq!(updated, expected);
    let check = |stage: &str| {
        let requirements = raw_rows(&fixture, "requirements", "req.jsonl");
        assert_eq!(requirements.len(), 2, "{stage}: {requirements:?}");
        assert_kept(&requirements, &other_requirements);
        assert_eq!(
            raw_rows(&fixture, "rules", "rule.jsonl"),
            rule_rows,
            "{stage}"
        );
        for requirement in fixture.store.list_requirements(&fixture.scope).unwrap() {
            assert_eq!(
                serde_json::to_value(&requirement).unwrap()["status"],
                "active",
                "{stage}"
            );
        }
        let rule =
            serde_json::to_value(&fixture.store.list_rules(&fixture.scope).unwrap()[0]).unwrap();
        assert_eq!(rule["status"], "active", "{stage}");
        let saved = fixture
            .store
            .list_requirements(&fixture.scope)
            .unwrap()
            .into_iter()
            .map(|requirement| serde_json::to_value(requirement).unwrap())
            .find(|requirement| requirement["id"] == id.as_str())
            .unwrap();
        assert_eq!(saved, expected, "{stage}");
    };
    check("after the owner edit");

    // The spec does not declare a description, so applying it again keeps
    // the owner's edit and changes nothing else.
    fixture.call("apply", spec).await.unwrap();
    check("after the second apply");
}
