use provenance_core::{
    Rule, RuleSeverity, RuleStatus, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};
use provenance_scanner::{
    derive_rule_evidence_facts, RuleEvidenceCompleteness,
};

fn rule(id: &str, status: RuleStatus) -> Rule {
    Rule {
        created: None,
        updated: None,
        archived_in_commit: None,
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        declared_by: None,
        declaration_address: None,
        name: None,
        description: None,
        statement: "The system groups claims by participant".to_string(),
        status,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

#[test]
fn incomplete_evidence_withholds_absence_facts() {
    let facts = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[],
        &[],
        &[],
        RuleEvidenceCompleteness::Incomplete,
    );

    assert!(facts.unimplemented.is_empty());
    assert!(facts.unverified.is_empty());
}
