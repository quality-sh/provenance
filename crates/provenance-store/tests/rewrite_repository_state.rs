// Temporary repository rewrite through the Store JSONL writer.
#[test]
fn rewrite_repository_state() {
    let input = include_str!("../../../.provenance/state/scopes/default/requirements/req.jsonl");
    let records: Vec<serde_json::Value> = input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
            if let Some(parent) = record
                .get("refines")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
            {
                record["refines"] = serde_json::json!([parent]);
            }
            // Validate each canonical record with the new model before publication.
            serde_json::from_value::<provenance_core::Requirement>(record.clone()).unwrap();
            record
        })
        .collect();
    let temp = tempfile::tempdir().unwrap();
    let path = camino::Utf8Path::from_path(temp.path())
        .unwrap()
        .join("requirements.jsonl");
    provenance_store::jsonl::write_jsonl_atomic(&path, &records).unwrap();
    for line in std::fs::read_to_string(path).unwrap().lines() {
        println!("REWRITTEN_RECORD={line}");
    }
}
