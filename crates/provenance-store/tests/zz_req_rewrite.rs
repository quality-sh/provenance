#[test]
fn rewrite_req_jsonl() {
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.provenance/state/scopes/default/requirements/req.jsonl");
    let records: Vec<provenance_core::Requirement> = std::fs::read_to_string(source)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(dir.path().join("req.jsonl")).unwrap();
    provenance_store::jsonl::write_jsonl_atomic(&path, &records).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    for line in written.lines() {
        println!("REQLINE:{line}");
    }
    println!("REQSAME:{}", written == std::fs::read_to_string(source).unwrap());
}
