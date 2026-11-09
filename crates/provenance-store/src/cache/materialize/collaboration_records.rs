use super::batch::{optional_text, text, Rows, Table};
use crate::cache::serde_name;
use provenance_core::model::ColumnValue;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, Message, ProposalCard, SynthesisPacket,
    Thread,
};
use serde::de::DeserializeOwned;

/// The rows of one collaboration family in `table`, one row for each
/// record, with the values that `row` gives.
fn rows_of<T: DeserializeOwned>(
    bytes: &[u8],
    table: &str,
    columns: &[&str],
    row: impl Fn(T) -> anyhow::Result<Vec<ColumnValue>>,
) -> anyhow::Result<Rows> {
    let columns: Vec<String> = columns.iter().map(|column| (*column).to_owned()).collect();
    let mut rows = Rows::new(vec![Table::insert(table, &columns)]);
    for record in serde_json::from_slice::<Vec<T>>(bytes)? {
        match row(record) {
            Ok(values) => {
                rows.push(0, values);
                rows.loaded += 1;
            }
            Err(error) => {
                rows.failure = Some(error);
                break;
            }
        }
    }
    Ok(rows)
}

fn json<T: serde::Serialize>(value: &T) -> anyhow::Result<ColumnValue> {
    Ok(text(serde_json::to_string(value)?))
}

pub(super) fn rows_of_threads(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "parent_type",
        "parent_id",
        "status",
        "created_at",
        "payload",
    ];
    rows_of(bytes, "threads", &columns, |thread: Thread| {
        let payload = json(&thread)?;
        Ok(vec![
            text(thread.scope_id.as_str()),
            text(thread.id.as_str()),
            text(serde_name(&thread.parent.node_type)?),
            text(thread.parent.node_id.as_str()),
            text(serde_name(&thread.status)?),
            ColumnValue::Integer(thread.created_at),
            payload,
        ])
    })
}

pub(super) fn rows_of_messages(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "thread_id",
        "role",
        "body",
        "created_at",
        "payload",
    ];
    rows_of(bytes, "messages", &columns, |message: Message| {
        let payload = json(&message)?;
        Ok(vec![
            text(message.scope_id.as_str()),
            text(message.id.as_str()),
            text(message.thread_id.as_str()),
            text(serde_name(&message.role)?),
            text(message.body),
            ColumnValue::Integer(message.created_at),
            payload,
        ])
    })
}

pub(super) fn rows_of_contributions(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "target_type",
        "target_id",
        "participant_slot",
        "stance",
        "strongest_finding",
        "uncertainty",
        "payload",
    ];
    rows_of(bytes, "contributions", &columns, |record: Contribution| {
        Ok(vec![
            text(record.scope_id.as_str()),
            text(record.id.as_str()),
            text(serde_name(&record.target.artifact_type)?),
            text(record.target.artifact_id.as_str()),
            text(record.participant_slot.as_str()),
            text(serde_name(&record.stance)?),
            text(record.strongest_finding.as_str()),
            json(&record.uncertainty)?,
            json(&record)?,
        ])
    })
}

pub(super) fn rows_of_synthesis_packets(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "target_type",
        "target_id",
        "summary",
        "payload",
    ];
    rows_of(
        bytes,
        "synthesis_packets",
        &columns,
        |packet: SynthesisPacket| {
            Ok(vec![
                text(packet.scope_id.as_str()),
                text(packet.id.as_str()),
                text(serde_name(&packet.target.artifact_type)?),
                text(packet.target.artifact_id.as_str()),
                text(packet.summary.as_str()),
                json(&packet)?,
            ])
        },
    )
}

pub(super) fn rows_of_assertion_records(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "proposal_id",
        "synthesis_packet_id",
        "supporting_claim_ids",
        "payload",
    ];
    rows_of(
        bytes,
        "assertion_records",
        &columns,
        |record: AssertionRecord| {
            Ok(vec![
                text(record.scope_id.as_str()),
                text(record.id.as_str()),
                text(record.proposal_id.as_str()),
                text(record.synthesis_packet_id.as_str()),
                json(&record.supporting_claim_ids)?,
                json(&record)?,
            ])
        },
    )
}

pub(super) fn rows_of_proposal_cards(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "proposal_key",
        "proposal_type",
        "title",
        "summary",
        "confidence",
        "target_type",
        "target_id",
        "traceability",
        "builds_on",
        "promotion_state",
        "duplicate_of",
        "superseded_by",
        "payload",
    ];
    rows_of(
        bytes,
        "proposal_cards",
        &columns,
        |proposal: ProposalCard| {
            let payload = json(&proposal)?;
            Ok(vec![
                text(proposal.scope_id.as_str()),
                text(proposal.id.as_str()),
                text(proposal.proposal_key.as_str()),
                text(serde_name(&proposal.proposal_type)?),
                text(proposal.title.as_str()),
                text(proposal.summary.as_str()),
                proposal
                    .confidence
                    .map_or(ColumnValue::Null, ColumnValue::Real),
                text(serde_name(&proposal.traceability.target.artifact_type)?),
                text(proposal.traceability.target.artifact_id.as_str()),
                json(&proposal.traceability)?,
                json(&proposal.builds_on)?,
                text(serde_name(&proposal.promotion_state)?),
                optional_text(
                    proposal
                        .duplicate_of
                        .as_ref()
                        .map(provenance_core::StableId::as_str),
                ),
                optional_text(
                    proposal
                        .superseded_by
                        .as_ref()
                        .map(provenance_core::StableId::as_str),
                ),
                payload,
            ])
        },
    )
}

pub(super) fn rows_of_dispositions(bytes: &[u8]) -> anyhow::Result<Rows> {
    let columns = [
        "scope_id",
        "id",
        "proposal_id",
        "decision",
        "rationale",
        "actor",
        "canonical_artifact",
        "external_action",
        "payload",
    ];
    rows_of(
        bytes,
        "dispositions",
        &columns,
        |record: DispositionRecord| {
            let payload = json(&record)?;
            Ok(vec![
                text(record.scope_id.as_str()),
                text(record.id.as_str()),
                text(record.proposal_id.as_str()),
                text(serde_name(&record.decision)?),
                text(record.rationale.as_str()),
                json(&record.actor)?,
                optional_text(
                    record
                        .canonical_artifact
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?,
                ),
                optional_text(
                    record
                        .external_action
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?,
                ),
                payload,
            ])
        },
    )
}
