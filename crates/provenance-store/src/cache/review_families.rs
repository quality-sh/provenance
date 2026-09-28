use provenance_core::{review::ReviewRecord, NodeType};
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, Copy)]
pub struct ReviewFamily {
    pub kind: NodeType,
    pub directory: &'static str,
    pub owner_field: &'static str,
    pub content_fields: &'static [&'static str],
    pub lifecycle_fields: &'static [&'static str],
}

pub const REVIEW_FAMILIES: [ReviewFamily; 8] = [
    ReviewFamily {
        kind: NodeType::Source,
        directory: "sources",
        owner_field: "id",
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
    },
    ReviewFamily {
        kind: NodeType::Requirement,
        directory: "requirements",
        owner_field: "id",
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
    },
    ReviewFamily {
        kind: NodeType::Resolution,
        directory: "resolutions",
        owner_field: "id",
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
    },
    ReviewFamily {
        kind: NodeType::Rule,
        directory: "rules",
        owner_field: "id",
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
    },
    ReviewFamily {
        kind: NodeType::Domain,
        directory: "domains",
        owner_field: "id",
        content_fields: &["name", "description"],
        lifecycle_fields: &["schema_version", "scope_id", "id", "color"],
    },
    ReviewFamily {
        kind: NodeType::Boundary,
        directory: "boundaries",
        owner_field: "id",
        content_fields: &["requirement_id", "statement", "source_ref"],
        lifecycle_fields: &["schema_version", "scope_id", "id"],
    },
    ReviewFamily {
        kind: NodeType::Topic,
        directory: "topics",
        owner_field: "id",
        content_fields: &["requirement_id", "title", "links"],
        lifecycle_fields: &[
            "schema_version",
            "scope_id",
            "id",
            "status",
            "claimed_by",
            "claimed_at",
        ],
    },
    ReviewFamily {
        kind: NodeType::Question,
        directory: "questions",
        owner_field: "id",
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
    },
];

pub fn by_kind(kind: NodeType) -> &'static ReviewFamily {
    REVIEW_FAMILIES
        .iter()
        .find(|family| family.kind == kind)
        .expect("all NodeType values have review-family facts")
}

pub fn by_directory(directory: &str) -> Option<&'static ReviewFamily> {
    REVIEW_FAMILIES
        .iter()
        .find(|family| family.directory == directory)
}

pub fn deserialize_record(
    kind: NodeType,
    value: serde_json::Value,
) -> anyhow::Result<ReviewRecord> {
    fn closed<T: DeserializeOwned>(value: &serde_json::Value) -> anyhow::Result<T> {
        let text = serde_json::to_string(&value)?;
        let mut unknown = None;
        let mut deserializer = serde_json::Deserializer::from_str(&text);
        let record = serde_ignored::deserialize(&mut deserializer, |path| {
            if unknown.is_none() {
                unknown = Some(path.to_string());
            }
        })?;
        anyhow::ensure!(
            unknown.is_none(),
            "unknown field `{}`",
            unknown.unwrap_or_default()
        );
        Ok(record)
    }
    Ok(match kind {
        NodeType::Source => ReviewRecord::Source(closed(&value)?),
        NodeType::Requirement => ReviewRecord::Requirement(closed(&value)?),
        NodeType::Resolution => ReviewRecord::Resolution(closed(&value)?),
        NodeType::Rule => ReviewRecord::Rule(closed(&value)?),
        NodeType::Domain => ReviewRecord::Domain(closed(&value)?),
        NodeType::Boundary => ReviewRecord::Boundary(closed(&value)?),
        NodeType::Topic => ReviewRecord::Topic(closed(&value)?),
        NodeType::Question => ReviewRecord::Question(closed(&value)?),
    })
}

#[cfg(test)]
mod tests {
    use super::{by_kind, REVIEW_FAMILIES};
    use provenance_core::NodeType;

    #[test]
    fn every_record_kind_has_one_review_family() {
        assert_eq!(REVIEW_FAMILIES.len(), NodeType::ALL.len());
        for kind in NodeType::ALL {
            let facts = by_kind(kind);
            assert_eq!(facts.kind, kind);
            assert_eq!(facts.owner_field, "id");
            assert!(!facts.content_fields.is_empty());
            assert!(!facts.lifecycle_fields.is_empty());
        }
    }

    #[test]
    fn common_audit_fields_are_lifecycle_fields_when_present() {
        let common = [
            "schema_version",
            "scope_id",
            "id",
            "created",
            "updated",
            "origin_thread",
            "origin_message",
        ];
        for family in REVIEW_FAMILIES.iter().take(4) {
            for field in common {
                assert!(family.lifecycle_fields.contains(&field));
            }
        }
    }
}
