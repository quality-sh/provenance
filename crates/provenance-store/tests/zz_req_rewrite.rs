#[test]
fn rewrite_req_jsonl() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.provenance/state/scopes/default/requirements/req.jsonl"
    ))
    .unwrap();
    let records: Vec<provenance_core::Requirement> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
            if let Some(parent) = value.get("refines").and_then(|v| v.as_str()).map(str::to_owned) {
                value["refines"] = serde_json::json!([parent]);
            }
            serde_json::from_value(value).unwrap()
        })
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(dir.path().join("req.jsonl")).unwrap();
    provenance_store::jsonl::write_jsonl_atomic(&path, &records).unwrap();
    for line in std::fs::read_to_string(&path).unwrap().lines() {
        println!("REQLINE:{line}");
    }
}
