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
        "supersedes",
    ],
    lifecycle_fields: &[
        "scope_id",
        "id",
        "created",
        "updated",
        "declared_by",
        "declaration_address",
        "origin_thread",
        "origin_message",
    ],
};
pub const REQUIREMENT: ReviewFields = ReviewFields {
    content_fields: &[
        "statement",
        "description",
        "source_refs",
        "refines",
        "depends_on",
        "supersedes",
        "spawned_by",
    ],
    lifecycle_fields: &[
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
pub const RESOLUTION: ReviewFields = ReviewFields {
    content_fields: &[
        "title",
        "position",
        "rationale",
        "context",
        "inputs",
        "requirement_ids",
        "supersedes",
    ],
    lifecycle_fields: &[
        "scope_id",
        "id",
        "created",
        "updated",
        "status",
        "archived_in_commit",
        "made_by",
        "approved_by",
        "approved_at",
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
pub const TOPIC: ReviewFields = ReviewFields {
    content_fields: &["requirement_id", "title", "links"],
    lifecycle_fields: &[
        "scope_id",
        "id",
        "status",
        "archived_in_commit",
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
        "depends_on",
        "answer",
        "links",
        "resolution_id",
        "contradicts",
    ],
    lifecycle_fields: &[
        "scope_id",
        "id",
        "status",
        "archived_in_commit",
        "claimed_by",
        "claimed_at",
    ],
};
