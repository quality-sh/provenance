//! Review field facts referenced by canonical record-family rows.

#[derive(Debug, Clone, Copy)]
pub struct ReviewFields {
    pub content_fields: &'static [&'static str],
    pub lifecycle_fields: &'static [&'static str],
}

pub const SOURCE: ReviewFields = ReviewFields {
    content_fields: &[
        "name",
        "source_type",
        "url",
        "reference",
        "commit_pin",
        "effective_date",
        "supersedes",
    ],
    lifecycle_fields: &[
        "schema_version",
        "scope_id",
        "id",
        "created",
        "updated",
        "declared_by",
        "declaration_address",
        "review_date",
        "origin_thread",
        "origin_message",
    ],
};
pub const REQUIREMENT: ReviewFields = ReviewFields {
    content_fields: &[
        "statement",
        "description",
        "fog",
        "domain_id",
        "source_refs",
        "refines",
        "depends_on",
        "supersedes",
        "spawned_by",
    ],
    lifecycle_fields: &[
        "schema_version",
        "scope_id",
        "id",
        "created",
        "updated",
        "declared_by",
        "declaration_address",
        "status",
        "origin_thread",
        "origin_message",
    ],
};
pub const RESOLUTION: ReviewFields = ReviewFields {
    content_fields: &[
        "title",
        "position",
        "rationale",
        "context",
        "enforcement",
        "confidence",
        "inputs",
        "requirement_ids",
        "supersedes",
    ],
    lifecycle_fields: &[
        "schema_version",
        "scope_id",
        "id",
        "created",
        "updated",
        "status",
        "made_by",
        "approved_by",
        "approved_at",
        "review_on",
        "origin_thread",
        "origin_message",
    ],
};
pub const RULE: ReviewFields = ReviewFields {
    content_fields: &[
        "name",
        "description",
        "statement",
        "severity",
        "requirement_ids",
        "resolution_ids",
        "source_document",
        "source_section",
    ],
    lifecycle_fields: &[
        "schema_version",
        "scope_id",
        "id",
        "created",
        "updated",
        "declared_by",
        "declaration_address",
        "status",
        "archived_in_commit",
        "origin_thread",
        "origin_message",
    ],
};
pub const DOMAIN: ReviewFields = ReviewFields {
    content_fields: &["name", "description"],
    lifecycle_fields: &["schema_version", "scope_id", "id", "color"],
};
pub const BOUNDARY: ReviewFields = ReviewFields {
    content_fields: &["requirement_id", "statement", "source_ref"],
    lifecycle_fields: &["schema_version", "scope_id", "id"],
};
pub const TOPIC: ReviewFields = ReviewFields {
    content_fields: &["requirement_id", "title", "links"],
    lifecycle_fields: &[
        "schema_version",
        "scope_id",
        "id",
        "status",
        "claimed_by",
        "claimed_at",
    ],
};
pub const QUESTION: ReviewFields = ReviewFields {
    content_fields: &[
        "topic_id",
        "requirement_id",
        "question",
        "resolution_method",
        "answer",
        "links",
        "resolution_id",
        "contradicts",
    ],
    lifecycle_fields: &[
        "schema_version",
        "scope_id",
        "id",
        "status",
        "claimed_by",
        "claimed_at",
    ],
};
