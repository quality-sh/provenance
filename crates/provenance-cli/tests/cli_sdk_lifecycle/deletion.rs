use super::*;

#[test]
fn deleting_a_requirement_removes_its_ids_from_surviving_records() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let owner = "spec://typescript/share-links";
    let mut input = document(owner);
    input["requirements"].as_array_mut().unwrap().push(json!({
        "key": "other", "statement": "Sessions expire"
    }));
    let initial = sdk(repo, "apply", &input).unwrap();
    let removed = resource_id(&initial, "requirement", "sharing");
    let retained = resource_id(&initial, "requirement", "other");
    let base = directory.path().join(".provenance/state/scopes/default");
    std::fs::create_dir_all(base.join("resolutions")).unwrap();
    std::fs::write(
        base.join("resolutions/res.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"schema_version": 2,"scope_id":"default","id":"res_shared",
            "title":"Shared decision","position":"Expire sessions","rationale":"Limit access",
            "status":"draft","requirement_ids":[removed,retained],"review_on":null}),
            json!({"schema_version": 2,"scope_id":"default","id":"res_removed",
            "title":"Removed decision","position":"Expire links","rationale":"Limit access",
            "status":"draft","requirement_ids":[removed],"review_on":null})
        ),
    )
    .unwrap();
    let mut rules = read_records(directory.path(), "rules/rule.jsonl");
    rules[0]["declared_by"] = Value::Null;
    rules[0]
        .as_object_mut()
        .unwrap()
        .remove("declaration_address");
    rules[0]["requirement_ids"] = json!([removed, retained]);
    rules[0]["resolution_ids"] = json!(["res_shared", "res_removed"]);
    std::fs::write(base.join("rules/rule.jsonl"), format!("{}\n", rules[0])).unwrap();
    input["rules"] = json!([]);
    input["requirements"].as_array_mut().unwrap().remove(0);

    sdk(repo, "apply", &input).unwrap();

    let rules = read_records(directory.path(), "rules/rule.jsonl");
    assert_eq!(rules[0]["requirement_ids"], json!([retained]));
    assert_eq!(rules[0]["resolution_ids"], json!(["res_shared"]));
    let resolutions = read_records(directory.path(), "resolutions/res.jsonl");
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0]["requirement_ids"], json!([retained]));
}
