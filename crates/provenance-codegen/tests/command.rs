//! The codegen command line writes the documents and the generated types.

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn export_writes_each_document_as_json_and_rust_reads_the_corpus_back() {
    let temporary = tempfile::tempdir().unwrap();
    let documents = temporary.path().join("documents");
    let documents_arg = documents.to_str().unwrap();

    provenance_codegen::run(&words(&["export", documents_arg])).unwrap();
    for name in [
        "openapi.json",
        "mcp.json",
        "compatibility.json",
        "fixtures.openapi.json",
    ] {
        let text = std::fs::read_to_string(documents.join(name)).unwrap();
        assert!(text.ends_with('\n'), "{name} must end with a newline");
        serde_json::from_str::<serde_json::Value>(&text)
            .unwrap_or_else(|error| panic!("{name} must be JSON: {error}"));
    }

    let generated = temporary.path().join("src/generated");
    let corpus = documents.join("fixtures.openapi.json");
    provenance_codegen::run(&words(&[
        "rust",
        corpus.to_str().unwrap(),
        generated.to_str().unwrap(),
    ]))
    .unwrap();
    let expected = provenance_codegen::rust_types(&provenance_codegen::corpus()).unwrap();
    assert!(!expected.is_empty());
    for (name, content) in expected {
        assert_eq!(
            std::fs::read_to_string(generated.join(&name)).unwrap(),
            content,
            "{name}"
        );
    }
}

#[test]
fn a_wrong_command_or_argument_count_returns_the_usage_text() {
    for args in [
        words(&[]),
        words(&["export"]),
        words(&["rust", "openapi.json"]),
        words(&["publish", "dir"]),
    ] {
        let error = provenance_codegen::run(&args).unwrap_err().to_string();
        assert!(
            error.starts_with("usage: provenance-codegen"),
            "{args:?}: {error}"
        );
    }
}
