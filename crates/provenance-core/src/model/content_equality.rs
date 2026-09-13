//! Record equality excludes machine-dependent creation and update stamps.
use super::{Requirement, Resolution, Rule, Source};

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.scope_id == other.scope_id
            && self.id == other.id
            && self.declared_by == other.declared_by
            && self.declaration_address == other.declaration_address
            && self.name == other.name
            && self.source_type == other.source_type
            && self.url == other.url
            && self.reference == other.reference
            && self.commit_pin == other.commit_pin
            && self.effective_date == other.effective_date
            && self.review_date == other.review_date
            && self.supersedes == other.supersedes
            && self.origin_thread == other.origin_thread
            && self.origin_message == other.origin_message
    }
}

impl PartialEq for Requirement {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.scope_id == other.scope_id
            && self.id == other.id
            && self.declared_by == other.declared_by
            && self.declaration_address == other.declaration_address
            && self.statement == other.statement
            && self.description == other.description
            && self.fog == other.fog
            && self.status == other.status
            && self.domain_id == other.domain_id
            && self.source_refs == other.source_refs
            && self.refines == other.refines
            && self.depends_on == other.depends_on
            && self.supersedes == other.supersedes
            && self.spawned_by == other.spawned_by
            && self.origin_thread == other.origin_thread
            && self.origin_message == other.origin_message
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.scope_id == other.scope_id
            && self.id == other.id
            && self.declared_by == other.declared_by
            && self.declaration_address == other.declaration_address
            && self.name == other.name
            && self.description == other.description
            && self.statement == other.statement
            && self.status == other.status
            && self.severity == other.severity
            && self.requirement_ids == other.requirement_ids
            && self.resolution_ids == other.resolution_ids
            && self.source_document == other.source_document
            && self.source_section == other.source_section
            && self.origin_thread == other.origin_thread
            && self.origin_message == other.origin_message
            && self.archived_in_commit == other.archived_in_commit
    }
}

impl PartialEq for Resolution {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.scope_id == other.scope_id
            && self.id == other.id
            && self.title == other.title
            && self.position == other.position
            && self.rationale == other.rationale
            && self.status == other.status
            && self.context == other.context
            && self.enforcement == other.enforcement
            && self.confidence == other.confidence
            && self.inputs == other.inputs
            && self.made_by == other.made_by
            && self.approved_by == other.approved_by
            && self.approved_at == other.approved_at
            && self.requirement_ids == other.requirement_ids
            && self.supersedes == other.supersedes
            && self.review_on == other.review_on
            && self.origin_thread == other.origin_thread
            && self.origin_message == other.origin_message
    }
}
