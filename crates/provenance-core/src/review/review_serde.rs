use super::{RecordSnapshot, ReviewEntry, ReviewRecord, SaveOutcome, SnapshotRef};
use crate::{NodeType, SchemaVersion, ScopeId, StableId};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewEntryWire {
    schema_version: SchemaVersion,
    scope_id: ScopeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    requirement_id: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    record_kind: Option<NodeType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    record_id: Option<StableId>,
    id: StableId,
    sequence: u64,
    predecessor: Option<StableId>,
    revision: StableId,
    prior_revision: Option<StableId>,
    before: Option<SnapshotRef>,
    after: SnapshotRef,
    changed_fields: Vec<String>,
    actor: String,
    request_id: StableId,
    intent_digest: String,
    etag: String,
    outcome: SaveOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<crate::threads::DiscussionOrigin>,
}

impl Serialize for ReviewEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let requirement = self.record_kind == NodeType::Requirement;
        ReviewEntryWire {
            schema_version: self.schema_version,
            scope_id: self.scope_id.clone(),
            requirement_id: requirement.then(|| self.record_id.clone()),
            record_kind: (!requirement).then_some(self.record_kind),
            record_id: (!requirement).then(|| self.record_id.clone()),
            id: self.id.clone(),
            sequence: self.sequence,
            predecessor: self.predecessor.clone(),
            revision: self.revision.clone(),
            prior_revision: self.prior_revision.clone(),
            before: self.before.clone(),
            after: self.after.clone(),
            changed_fields: self.changed_fields.clone(),
            actor: self.actor.clone(),
            request_id: self.request_id.clone(),
            intent_digest: self.intent_digest.clone(),
            etag: self.etag.clone(),
            outcome: self.outcome,
            origin: self.origin.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ReviewEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = ReviewEntryWire::deserialize(deserializer)?;
        let (record_kind, record_id) = if let Some(id) = wire.requirement_id {
            if wire.record_kind.is_some() || wire.record_id.is_some() {
                return Err(D::Error::custom("invalid review record address"));
            }
            (NodeType::Requirement, id)
        } else {
            let kind = wire
                .record_kind
                .ok_or_else(|| D::Error::custom("invalid review record address"))?;
            let id = wire
                .record_id
                .ok_or_else(|| D::Error::custom("invalid review record address"))?;
            if kind == NodeType::Requirement {
                return Err(D::Error::custom("invalid review record address"));
            }
            (kind, id)
        };
        Ok(Self {
            schema_version: wire.schema_version,
            scope_id: wire.scope_id,
            record_kind,
            record_id,
            id: wire.id,
            sequence: wire.sequence,
            predecessor: wire.predecessor,
            revision: wire.revision,
            prior_revision: wire.prior_revision,
            before: wire.before,
            after: wire.after,
            changed_fields: wire.changed_fields,
            actor: wire.actor,
            request_id: wire.request_id,
            intent_digest: wire.intent_digest,
            etag: wire.etag,
            outcome: wire.outcome,
            origin: wire.origin,
        })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum RecordRef<'a> {
    Source(&'a crate::Source),
    Requirement(&'a crate::Requirement),
    Resolution(&'a crate::Resolution),
    Rule(&'a crate::Rule),
    Domain(&'a crate::Domain),
    Boundary(&'a crate::Boundary),
    Topic(&'a crate::Topic),
    Question(&'a crate::Question),
}

impl<'a> From<&'a ReviewRecord> for RecordRef<'a> {
    fn from(record: &'a ReviewRecord) -> Self {
        match record {
            ReviewRecord::Source(value) => Self::Source(value),
            ReviewRecord::Requirement(value) => Self::Requirement(value),
            ReviewRecord::Resolution(value) => Self::Resolution(value),
            ReviewRecord::Rule(value) => Self::Rule(value),
            ReviewRecord::Domain(value) => Self::Domain(value),
            ReviewRecord::Boundary(value) => Self::Boundary(value),
            ReviewRecord::Topic(value) => Self::Topic(value),
            ReviewRecord::Question(value) => Self::Question(value),
        }
    }
}

impl Serialize for ReviewRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RecordRef::from(self).serialize(serializer)
    }
}

#[derive(Serialize)]
struct RecordSnapshotRef<'a> {
    schema_version: SchemaVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_kind: Option<NodeType>,
    record: RecordRef<'a>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordSnapshotWire {
    schema_version: SchemaVersion,
    #[serde(default)]
    record_kind: Option<NodeType>,
    record: serde_json::Value,
}

impl Serialize for RecordSnapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = self.record.kind();
        RecordSnapshotRef {
            schema_version: self.schema_version,
            record_kind: (kind != NodeType::Requirement).then_some(kind),
            record: (&self.record).into(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RecordSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = RecordSnapshotWire::deserialize(deserializer)?;
        let kind = wire.record_kind.unwrap_or(NodeType::Requirement);
        let record = match kind {
            NodeType::Source => serde_json::from_value(wire.record).map(ReviewRecord::Source),
            NodeType::Requirement => {
                serde_json::from_value(wire.record).map(ReviewRecord::Requirement)
            }
            NodeType::Resolution => {
                serde_json::from_value(wire.record).map(ReviewRecord::Resolution)
            }
            NodeType::Rule => serde_json::from_value(wire.record).map(ReviewRecord::Rule),
            NodeType::Domain => serde_json::from_value(wire.record).map(ReviewRecord::Domain),
            NodeType::Boundary => serde_json::from_value(wire.record).map(ReviewRecord::Boundary),
            NodeType::Topic => serde_json::from_value(wire.record).map(ReviewRecord::Topic),
            NodeType::Question => serde_json::from_value(wire.record).map(ReviewRecord::Question),
        }
        .map_err(D::Error::custom)?;
        Ok(Self {
            schema_version: wire.schema_version,
            record,
        })
    }
}
