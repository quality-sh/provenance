//! Bounded pages of the canonical JSON of one record version.

use crate::{canonical_digest, operations::reader::RECORD_BYTES};
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_core::{review::EvidencePage, StableId};

/// The most JSON text bytes one evidence page returns.
const PAGE_TEXT_BYTES: u64 = 8192;

/// Pages the canonical JSON of `value`, the selected record version or one
/// field of it, from `offset`, ending each page at a UTF-8 boundary.
pub(in crate::review) fn page(
    version: StableId,
    value: &serde_json::Value,
    field: Option<String>,
    offset: u64,
) -> anyhow::Result<EvidencePage> {
    let document = canonical_digest::canonical_bytes(value)?;
    let length = document.len() as u64;
    if offset >= length {
        return Err(OperationFailure::InvalidInput {
            field: Some("offset".to_owned()),
            reason: InvalidInputReason::InvalidValue,
        }
        .into());
    }
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
