//! Bounded pages of the canonical JSON of one record version.

use crate::{canonical_digest, operations::reader::RECORD_BYTES};
use provenance_core::{review::EvidencePage, StableId};

/// The most JSON text bytes one evidence page returns.
const PAGE_TEXT_BYTES: u64 = 8192;

/// Pages the canonical JSON of a record version, or of one of its fields,
/// from `offset`, ending each page at a UTF-8 boundary.
pub(in crate::review) fn page(
    version: StableId,
    record: &serde_json::Value,
    field: Option<String>,
    offset: u64,
) -> anyhow::Result<EvidencePage> {
    let document = match &field {
        Some(name) => canonical_digest::canonical_bytes(
            record
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("record version has no field {name}"))?,
        )?,
        None => canonical_digest::canonical_bytes(record)?,
    };
    let length = document.len() as u64;
    anyhow::ensure!(
        offset < length,
        "evidence offset is outside the selected record version field"
    );
    let start = usize::try_from(offset)?;
    let end = usize::try_from((length - offset).min(PAGE_TEXT_BYTES) + offset)?;
    let buffer = &document[start..end];
    let text = match std::str::from_utf8(buffer) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() && error.valid_up_to() > 0 => {
            std::str::from_utf8(&buffer[..error.valid_up_to()])?
        }
        Err(_) => anyhow::bail!("evidence offset is not a UTF-8 boundary"),
    };
    let next = offset + text.len() as u64;
    let result = EvidencePage {
        version,
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
