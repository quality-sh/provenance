use super::artifacts::{Requirement, Resolution, Rule, Source};

#[test]
fn declaration_retirement_is_explicit_and_backward_compatible() {
    let source: Source = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "source_policy",
        "name": "Policy",
        "source_type": "policy",
        "url": null,
        "retired": true
    }))
    .unwrap();
    let requirement: Requirement = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "requirement_sharing",
        "statement": "Sharing is controlled",
        "status": "active"
    }))
    .unwrap();
    let rule: Rule = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "rule_expiry",
        "statement": "Shares expire",
        "status": "active",
        "severity": "medium",
        "retired": true
    }))
    .unwrap();

    assert!(source.retired);
    assert!(!requirement.retired);
    assert!(rule.retired);
    assert_eq!(serde_json::to_value(source).unwrap()["retired"], true);
    assert!(serde_json::to_value(requirement)
        .unwrap()
        .get("retired")
        .is_none());
}

#[test]
fn enriched_source_and_requirement_records_roundtrip_without_schema_bump() {
    let source = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "source_sah",
        "name": "Support at Home",
        "source_type": "legislation",
        "url": "https://example.test/sah",
        "reference": "Department guidance",
        "commitPin": "5e1f2a9c4b6d8e0f1234567890abcdef12345678",
        "effectiveDate": 1_714_521_600_000_i64,
        "reviewDate": 1_717_200_000_000_i64,
        "supersedes": ["source_sah_2025"],
        "originThread": "thread_req_origin",
        "originMessage": "msg_000001"
    });
    let requirement = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "req_sah",
        "statement": "Support at Home shall be traceable",
        "description": "Cloud import description",
        "status": "discovery",
        "source_refs": [{"source_id": "source_sah", "clause": "Program overview"}],
        "originThread": "thread_req_origin",
        "originMessage": "msg_000001"
    });

    let source: Source = serde_json::from_value(source).unwrap();
    let requirement: Requirement = serde_json::from_value(requirement).unwrap();

    let source = serde_json::to_value(source).unwrap();
    let requirement = serde_json::to_value(requirement).unwrap();

    assert_eq!(source["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(source["source_type"], "legislation");
    assert_eq!(source["reference"], "Department guidance");
    assert_eq!(
        source["commit_pin"],
        "5e1f2a9c4b6d8e0f1234567890abcdef12345678"
    );
    assert_eq!(source["effective_date"], 1_714_521_600_000_i64);
    assert_eq!(source["review_date"], 1_717_200_000_000_i64);
    assert_eq!(source["supersedes"][0], "source_sah_2025");
    assert_eq!(source["origin_thread"], "thread_req_origin");
    assert_eq!(source["origin_message"], "msg_000001");
    assert_eq!(requirement["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(requirement["status"], "discovery");
    assert_eq!(requirement["description"], "Cloud import description");
    assert_eq!(requirement["source_refs"][0]["clause"], "Program overview");
    assert_eq!(requirement["origin_thread"], "thread_req_origin");
    assert_eq!(requirement["origin_message"], "msg_000001");
}

#[test]
#[verifies("rule_source_commit_pin", examples)]
fn source_commit_pin_must_be_hex_git_commit() {
    let source = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "source_codebase",
        "name": "Codebase",
        "source_type": "project_artifact",
        "commitPin": "main"
    });

    assert!(serde_json::from_value::<Source>(source)
        .unwrap_err()
        .to_string()
        .contains("commit pin"));
}

#[test]
fn enriched_resolution_and_rule_records_roundtrip_without_schema_bump() {
    let resolution = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "res_sah",
        "title": "SAH extraction",
        "position": "Keep as draft extraction",
        "rationale": "Needs human review",
        "status": "draft",
        "review_on": null,
        "context": "Codebase scan",
        "enforcement": "specification",
        "confidence": 0.91,
        "inputs": [{
            "inputType": "regulatory",
            "reference": "SAH program manual",
            "summary": "Program rules reviewed"
        }],
        "madeBy": "Analyst One",
        "approvedBy": "Approver Two",
        "approvedAt": 1_714_780_800_000_i64,
        "supersedes": ["res_sah_2025"],
        "originThread": "thread_req_origin",
        "originMessage": "msg_000001"
    });
    let rule = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "rule_sah_001",
        "name": "SAH rule",
        "description": "Rule description",
        "statement": "Draft rule shall stay draft",
        "status": "draft",
        "severity": "high",
        "source_document": "Example-API-main/src/example.php",
        "source_section": "lines 1-3",
        "originThread": "thread_req_origin",
        "originMessage": "msg_000001"
    });

    let resolution: Resolution = serde_json::from_value(resolution).unwrap();
    let rule: Rule = serde_json::from_value(rule).unwrap();

    let resolution = serde_json::to_value(resolution).unwrap();
    let rule = serde_json::to_value(rule).unwrap();

    assert_eq!(resolution["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(resolution["status"], "draft");
    assert_eq!(resolution["confidence"], 0.91);
    assert_eq!(resolution["inputs"][0]["input_type"], "regulatory");
    assert_eq!(resolution["inputs"][0]["reference"], "SAH program manual");
    assert_eq!(resolution["inputs"][0]["summary"], "Program rules reviewed");
    assert_eq!(resolution["made_by"], "Analyst One");
    assert_eq!(resolution["approved_by"], "Approver Two");
    assert_eq!(resolution["approved_at"], 1_714_780_800_000_i64);
    assert_eq!(resolution["supersedes"][0], "res_sah_2025");
    assert_eq!(resolution["origin_thread"], "thread_req_origin");
    assert_eq!(resolution["origin_message"], "msg_000001");
    assert_eq!(rule["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(rule["status"], "draft");
    assert_eq!(rule["source_document"], "Example-API-main/src/example.php");
    assert_eq!(rule["origin_thread"], "thread_req_origin");
    assert_eq!(rule["origin_message"], "msg_000001");
}

#[test]
fn requirement_fog_roundtrips_as_unstructured_text() {
    let requirement = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "req_share_links",
        "statement": "Provenance docs shareable via short-lived link",
        "status": "discovery",
        "fog": "access auditing; expiry configuration; something about revocation"
    });

    let requirement: Requirement = serde_json::from_value(requirement).unwrap();
    assert_eq!(
        requirement.fog.as_deref(),
        Some("access auditing; expiry configuration; something about revocation")
    );

    let requirement = serde_json::to_value(requirement).unwrap();
    assert_eq!(
        requirement["fog"],
        "access auditing; expiry configuration; something about revocation"
    );

    let without_fog = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "req_plain",
        "statement": "Plain requirement",
        "status": "active"
    });
    let without_fog: Requirement = serde_json::from_value(without_fog).unwrap();
    assert_eq!(without_fog.fog, None);
    assert!(serde_json::to_value(without_fog)
        .unwrap()
        .get("fog")
        .is_none());
}

/// Rule `expression`/`inputs` and resolution `review_triggers` were Statesman
/// leftovers, never written with content and never read. Rule `rule_code`,
/// `rule_type`, `modality`, `confidence` and `extraction_method` went the same
/// way: a second name for a rule nothing looked up, and four classifications
/// nothing branched on. They are gone. An export taken before the deletion
/// still carries them, and reading one drops them rather than failing: a hard
/// failure would strand old exports for no gain. The keys do not come back out
/// the other side.
#[test]
fn pre_deletion_exports_carrying_the_removed_keys_read_and_drop_them() {
    let resolution = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "res_legacy",
        "title": "Legacy export",
        "position": "Keep",
        "rationale": "Kept",
        "status": "approved",
        "inputs": [],
        "review_on": null,
        "review_triggers": []
    });
    let rule = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "rule_legacy",
        "rule_code": "LEGACY-001",
        "statement": "Legacy rule stays legible",
        "status": "active",
        "severity": "high",
        "expression": {},
        "inputs": [],
        "rule_type": "business",
        "modality": "obligation",
        "confidence": 0.98,
        "extraction_method": "manual"
    });

    let resolution: Resolution = serde_json::from_value(resolution).unwrap();
    let rule: Rule = serde_json::from_value(rule).unwrap();

    let resolution = serde_json::to_value(resolution).unwrap();
    let rule = serde_json::to_value(rule).unwrap();

    assert!(resolution.get("review_triggers").is_none());
    assert!(rule.get("expression").is_none());
    assert!(rule.get("inputs").is_none());
    assert!(rule.get("rule_code").is_none());
    assert!(rule.get("rule_type").is_none());
    assert!(rule.get("modality").is_none());
    assert!(rule.get("confidence").is_none());
    assert!(rule.get("extraction_method").is_none());
    assert_eq!(resolution["status"], "approved");
    assert_eq!(rule["statement"], "Legacy rule stays legible");
}

/// A resolution carrying a blank input is refused whole, on the way in. The
/// record is the unit of refusal: there is no half-read resolution with the
/// bad input dropped, because dropping it would leave a decision claiming
/// inputs it does not have.
#[test]
#[verifies("rule_resolution_input_content", examples)]
fn a_resolution_with_a_blank_input_field_is_refused_on_read() {
    let with_blank_reference = resolution_with_input("", "Program rules reviewed");
    let with_blank_summary = resolution_with_input("SAH program manual", "   ");
    let with_both = resolution_with_input("SAH program manual", "Program rules reviewed");

    assert_eq!(
        serde_json::from_value::<Resolution>(with_blank_reference)
            .unwrap_err()
            .to_string(),
        "resolution input reference must not be blank"
    );
    assert_eq!(
        serde_json::from_value::<Resolution>(with_blank_summary)
            .unwrap_err()
            .to_string(),
        "resolution input summary must not be blank"
    );
    let accepted: Resolution = serde_json::from_value(with_both).unwrap();
    assert_eq!(accepted.inputs[0].reference, "SAH program manual");
}

fn resolution_with_input(reference: &str, summary: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "res_sah",
        "title": "SAH extraction",
        "position": "Keep as draft extraction",
        "rationale": "Needs human review",
        "status": "draft",
        "review_on": null,
        "inputs": [{
            "inputType": "regulatory",
            "reference": reference,
            "summary": summary
        }]
    })
}
