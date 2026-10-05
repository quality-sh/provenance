use crate::canonical_digest;
use provenance_core::review::{ReviewRecord, SnapshotField};

pub(super) fn fields(record: &ReviewRecord) -> anyhow::Result<Vec<SnapshotField>> {
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
