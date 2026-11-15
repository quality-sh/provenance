//! Hash record content without the local Git creation and update metadata.
use camino::Utf8Path;

/// The bytes a content digest covers for one file. Catch-up keeps the stored
/// content digest of a unit whose bytes have not changed. After a change to
/// what this function covers, `provenance materialize` rebuilds the digests.
/// A line that is not JSON is refused with its file and its line.
pub(super) fn hash_bytes(path: &Utf8Path, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    if path.extension() != Some("jsonl")
        || !matches!(
            path.parent().and_then(Utf8Path::file_name),
            Some("sources" | "requirements" | "rules" | "resolutions")
        )
    {
        return Ok(bytes.to_vec());
    }
    let mut content = Vec::new();
    for (index, line) in bytes
        .split(|b| *b == b'\n')
        .enumerate()
        .filter(|(_, line)| !line.iter().all(u8::is_ascii_whitespace))
    {
        let mut record: serde_json::Value = serde_json::from_slice(line)
            .map_err(|error| crate::state_store::readers::located(path, index + 1, &error))?;
        if let Some(record) = record.as_object_mut() {
            record.remove("created");
            record.remove("updated");
        }
        content.extend(crate::canonical_digest::canonical_bytes(&record)?);
        content.push(b'\n');
    }
    Ok(content)
}
