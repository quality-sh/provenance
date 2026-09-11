use super::journal;
use crate::{canonical_digest, operations::reader::RECORD_BYTES, state_store::StateStore};
use provenance_core::review::{EvidencePage, SnapshotField, SnapshotRef};
use provenance_core::{Requirement, ScopeId};
use std::io::{Read, Seek, SeekFrom};

pub(super) fn fields(record: &Requirement) -> anyhow::Result<Vec<SnapshotField>> {
    let value = serde_json::to_value(record)?;
    let mut position = b"{\"record\":{".len() as u64;
    let mut result = Vec::new();
    for (index, (name, value)) in value.as_object().unwrap().iter().enumerate() {
        if index != 0 {
            position += 1;
        }
        position += serde_json::to_vec(name)?.len() as u64 + 1;
        let bytes = canonical_digest::canonical_bytes(value)?.len() as u64;
        result.push(SnapshotField {
            name: name.clone(),
            offset: position,
            bytes,
        });
        position += bytes;
    }
    Ok(result)
}

pub(super) fn evidence(
    store: &StateStore,
    scope: &ScopeId,
    reference: SnapshotRef,
    field: Option<String>,
    offset: u64,
) -> anyhow::Result<EvidencePage> {
    let path = journal::snapshot_path(&store.layout, scope, &reference.id);
    let mut file = journal::regular_file(&path)?;
    anyhow::ensure!(
        file.metadata()?.len() == reference.bytes,
        "immutable snapshot length mismatch"
    );
    let (base, length) = if let Some(name) = &field {
        let range = reference
            .fields
            .iter()
            .find(|f| f.name == *name)
            .ok_or_else(|| anyhow::anyhow!("snapshot has no field {name}"))?;
        (range.offset, range.bytes)
    } else {
        (0, reference.bytes)
    };
    anyhow::ensure!(
        base.checked_add(length)
            .is_some_and(|end| end <= reference.bytes),
        "invalid snapshot field range"
    );
    anyhow::ensure!(
        offset < length,
        "evidence offset is outside the selected snapshot field"
    );
    file.seek(SeekFrom::Start(base + offset))?;
    let mut buffer = Vec::with_capacity(8192);
    file.take((length - offset).min(8192))
        .read_to_end(&mut buffer)?;
    anyhow::ensure!(
        !buffer.is_empty(),
        "snapshot ended before its recorded length"
    );
    let text = match std::str::from_utf8(&buffer) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() && error.valid_up_to() > 0 => {
            std::str::from_utf8(&buffer[..error.valid_up_to()])?
        }
        Err(_) => anyhow::bail!("evidence offset is not a UTF-8 boundary or snapshot is invalid"),
    };
    let next = offset + text.len() as u64;
    let result = EvidencePage {
        snapshot: reference,
        offset,
        field,
        json_text: text.to_owned(),
        next_offset: (next < length).then_some(next),
    };
    anyhow::ensure!(
        serde_json::to_vec(&result)?.len() <= RECORD_BYTES,
        "evidence page exceeds the encoded byte budget"
    );
    Ok(result)
}
