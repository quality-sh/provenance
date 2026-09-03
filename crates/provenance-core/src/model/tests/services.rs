use super::services::Domain;

#[test]
fn domain_records_roundtrip_without_hosted_fields() {
    let domain = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "domain_payroll",
        "name": "Payroll",
        "description": "Payroll compliance requirements",
        "color": "#3b82f6"
    });
    let requirement = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "req_overtime",
        "statement": "Overtime must be traceable",
        "status": "discovery",
        "domainId": "domain_payroll"
    });

    let domain: Domain = serde_json::from_value(domain).unwrap();
    let requirement: Requirement = serde_json::from_value(requirement).unwrap();

    let domain = serde_json::to_value(domain).unwrap();
    let requirement = serde_json::to_value(requirement).unwrap();

    assert_eq!(domain["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(domain["id"], "domain_payroll");
    assert!(domain.get("createdBy").is_none());
    assert!(domain.get("updatedAt").is_none());
    assert_eq!(requirement["domain_id"], "domain_payroll");
}
