use super::*;

fn arguments(words: &[&str]) -> Vec<String> {
    std::iter::once("provenance")
        .chain(words.iter().copied())
        .map(str::to_owned)
        .collect()
}

#[test]
fn route_is_one_deterministic_grammar_decision() {
    let route = |words: &[&str]| CommandFamily::select(&arguments(words)).unwrap();
    assert_eq!(route(&["--help"]), CommandFamily::Builtin);
    assert_eq!(
        route(&["--quiet", "check", "--repo", "repo"]),
        CommandFamily::Builtin
    );
    assert_eq!(
        route(&["--repo", "repo", "sources", "list"]),
        CommandFamily::Catalog
    );
    assert_eq!(
        route(&["sources", "get", "--repo", "repo"]),
        CommandFamily::Get
    );
    for words in [
        &["sources", "get", "update", "--repo", "repo"][..],
        &["sources", "get", "get", "--repo", "repo"],
        &["sources", "update", "get", "--repo", "repo"],
    ] {
        assert_eq!(route(words), CommandFamily::Catalog);
    }
    for words in [
        &["sources", "update", "--name", "Changed"][..],
        &["sources", "create", "--type", "source"],
        &["topics", "claim", "--actor", "worker"],
    ] {
        assert_eq!(route(words), CommandFamily::Target);
    }
    assert_eq!(
        route(&["sources", "create", "--id", "source_legacy"]),
        CommandFamily::Catalog
    );
    assert_eq!(
        route(&["sources", "get", "--unknown-option", "value"]),
        CommandFamily::Get
    );
    assert_eq!(
        route(&["sources", "--help", "--repo", "repo"]),
        CommandFamily::Catalog
    );
    for words in [&["check", "--repo", "repo"][..], &["check", "--strict"]] {
        assert_eq!(route(words), CommandFamily::Builtin);
    }
    assert_eq!(
        route(&["check", "get", "--repo", "repo"]),
        CommandFamily::Get
    );
    assert_eq!(
        route(&["check", "--repo", "repo", "get", "--format", "json"]),
        CommandFamily::Get
    );
    assert_eq!(
        route(&["search", "--text", "needle"]),
        CommandFamily::Search
    );
    assert_eq!(
        route(&["--repo", "repo", "search", "--kind", "rule"]),
        CommandFamily::Search
    );
    assert_eq!(route(&["search", "--help"]), CommandFamily::Search);
    assert_eq!(
        route(&["search", "get", "--repo", "repo"]),
        CommandFamily::Get
    );
    assert_eq!(
        route(&["search", "update", "--name", "Changed"]),
        CommandFamily::Target
    );
    assert_eq!(
        route(&["--repo", "repo", "unknown_id", "--format", "json"]),
        CommandFamily::Get
    );
    assert_eq!(
        route(&["--repo", "repo", "unknown_id", "update"]),
        CommandFamily::Target
    );
    assert_eq!(
        route(&[
            "unknown_id",
            "--scope",
            "other",
            "create",
            "--type",
            "source",
        ]),
        CommandFamily::Target
    );
}

#[test]
fn local_flag_values_are_not_reparsed_as_globals() {
    let input = arguments(&[
        "sources",
        "create",
        "--name",
        "--repo",
        "--repo",
        "repository",
    ]);
    assert_eq!(
        CommandFamily::select(&input).unwrap(),
        CommandFamily::Catalog
    );
    let shared = ExternalArguments::parse(&input).unwrap();
    assert_eq!(shared.context.repo, "repository");
    assert_eq!(shared.words, ["sources", "create", "--name", "--repo"]);
}
