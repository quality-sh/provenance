use camino::Utf8PathBuf;
use serde::{de::DeserializeOwned, Serialize};

fn rewrite<T: DeserializeOwned + Serialize>(shard: &str, marker: &str) {
    let root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.provenance/state/scopes/default");
    let text = std::fs::read_to_string(root.join(shard)).unwrap();
    let records: Vec<T> = text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap()).collect();
    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("out.jsonl")).unwrap();
    provenance_store::jsonl::write_jsonl_atomic(&out, &records).unwrap();
    let written = std::fs::read_to_string(&out).unwrap();
    println!("{marker}SAME:{}", written == text);
    for line in written.lines() {
        println!("{marker}:{line}");
    }
}

#[test]
fn merge_rewrite() {
    rewrite::<provenance_core::Requirement>("requirements/req.jsonl", "REQLINE");
    rewrite::<provenance_core::Rule>("rules/rule.jsonl", "RULELINE");
    rewrite::<provenance_core::Source>("sources/source.jsonl", "SRCLINE");
    rewrite::<provenance_core::ProposalCard>("ideation/proposal_cards.jsonl", "CARDLINE");
}
