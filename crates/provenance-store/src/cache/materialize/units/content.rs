//! Hash record content without the local Git creation and update metadata.
use camino::Utf8Path;

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
    for line in bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
    {
        let mut record: serde_json::Value = serde_json::from_slice(line)?;
        if let Some(record) = record.as_object_mut() {
            record.remove("created");
            record.remove("updated");
        }
        content.extend(crate::canonical_digest::canonical_bytes(&record)?);
        content.push(b'\n');
    }
    Ok(content)
}
