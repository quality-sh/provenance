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
    write_dependent_records(&base, &removed, &retained);
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

    let planned = sdk(repo, "plan", &input).unwrap();
    let cascade = planned["cascade"]
        .as_array()
        .expect("plan reports cascade changes");
    for (kind, id, state) in [
        ("resolution", "res_shared", "updated"),
        ("resolution", "res_removed", "deleted"),
        ("rule", rules[0]["id"].as_str().unwrap(), "updated"),
        ("topic", "topic_removed", "deleted"),
        ("topic", "topic_retained", "updated"),
        ("question", "question_removed", "deleted"),
        ("question", "question_topic_removed", "deleted"),
        ("question", "question_retained", "updated"),
    ] {
        assert!(cascade.iter().any(|record| {
            record["kind"] == kind && record["id"] == id && record["state"] == state
        }));
    }
    assert_eq!(planned["updated"], 4);
    assert_eq!(planned["deleted"], 5);

    let applied = sdk(repo, "apply", &input).unwrap();
    assert_eq!(applied["cascade"], planned["cascade"]);
    assert_eq!(applied["updated"], planned["updated"]);
    assert_eq!(applied["deleted"], planned["deleted"]);

    let rules = read_records(directory.path(), "rules/rule.jsonl");
    assert_eq!(rules[0]["requirement_ids"], json!([retained]));
    assert_eq!(rules[0]["resolution_ids"], json!(["res_shared"]));
    let resolutions = read_records(directory.path(), "resolutions/res.jsonl");
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0]["requirement_ids"], json!([retained]));
    let topics = read_records(directory.path(), "topics/topic.jsonl");
    assert_eq!(topics.len(), 1);
    assert_eq!(
        topics[0]["links"],
        json!([{"target_type":"requirement","target_id":retained}])
    );
    let questions = read_records(directory.path(), "questions/question.jsonl");
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0]["id"], "question_retained");
    assert_eq!(questions[0]["contradicts"], Value::Null);
    assert_eq!(questions[0]["resolution_id"], Value::Null);
    assert_eq!(
        questions[0]["links"],
        json!([{"target_type":"requirement","target_id":retained}])
    );
}

fn write_dependent_records(base: &std::path::Path, removed: &str, retained: &str) {
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
    std::fs::create_dir_all(base.join("topics")).unwrap();
    std::fs::write(
        base.join("topics/topic.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"schema_version": 2,"scope_id":"default","id":"topic_removed",
            "requirement_id":removed,"title":"Removed topic","status":"open",
            "links":[]}),
            json!({"schema_version": 2,"scope_id":"default","id":"topic_retained",
            "requirement_id":retained,"title":"Retained topic","status":"open",
            "links":[
                {"target_type":"requirement","target_id":removed},
                {"target_type":"resolution","target_id":"res_removed"},
                {"target_type":"requirement","target_id":retained}
            ]})
        ),
    )
    .unwrap();
    std::fs::create_dir_all(base.join("questions")).unwrap();
    std::fs::write(
        base.join("questions/question.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            json!({"schema_version": 2,"scope_id":"default","id":"question_removed",
            "topic_id":"topic_retained","requirement_id":removed,
            "question":"Removed?","resolution_method":"research","status":"open"}),
            json!({"schema_version": 2,"scope_id":"default","id":"question_topic_removed",
            "topic_id":"topic_removed","requirement_id":retained,
            "question":"Also removed?","resolution_method":"research","status":"open"}),
            json!({"schema_version": 2,"scope_id":"default","id":"question_retained",
            "topic_id":"topic_retained","requirement_id":retained,
            "question":"Retained?","resolution_method":"research","status":"open",
            "contradicts":removed,"resolution_id":"res_removed","links":[
                {"target_type":"requirement","target_id":removed},
                {"target_type":"resolution","target_id":"res_removed"},
                {"target_type":"requirement","target_id":retained}
            ]})
        ),
    )
    .unwrap();
}

#[test]
fn a_disposition_refuses_canonical_deletion_before_any_shard_write() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let owner = "spec://typescript/share-links";
    let initial = sdk(repo, "apply", &document(owner)).unwrap();
    let requirement = resource_id(&initial, "requirement", "sharing");
    let base = directory.path().join(".provenance/state/scopes/default");
    std::fs::create_dir_all(base.join("ideation")).unwrap();
    std::fs::write(
        base.join("ideation/dispositions.jsonl"),
        format!(
            "{}\n",
            json!({"schema_version": 2,"scope_id":"default","id":"disposition_keep",
            "proposal_id":"proposal_keep","decision":"rejected","rationale":"Keep it",
            "actor":{"identity_type":"human","id":"reviewer"},
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":requirement}})
        ),
    )
    .unwrap();
    let before = [
        "sources/source.jsonl",
        "requirements/req.jsonl",
        "rules/rule.jsonl",
    ]
    .map(|path| (path, std::fs::read(base.join(path)).unwrap()));

    let error = sdk(repo, "apply", &empty_document(owner)).unwrap_err();

    assert!(error.contains("disposition_keep"), "{error}");
    assert!(error.contains("requirement"), "{error}");
    assert!(error.contains(&requirement), "{error}");
    for (path, contents) in before {
        assert_eq!(std::fs::read(base.join(path)).unwrap(), contents, "{path}");
    }
}
