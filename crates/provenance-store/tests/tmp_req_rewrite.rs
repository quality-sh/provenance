#[test]
fn rewrite() {
    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(
        root.join(".provenance/state/scopes/default/requirements/req.jsonl"),
    )
    .unwrap();
    let records: Vec<provenance_core::Requirement> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let temp = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(temp.path().join("requirements/req.jsonl")).unwrap();
    provenance_store::jsonl::write_jsonl_atomic(&path, &records).unwrap();
    for line in std::fs::read_to_string(&path).unwrap().lines() {
        println!("REQLINE:{line}");
    }
}
