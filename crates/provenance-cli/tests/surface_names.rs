//! Characterization of the two command surfaces, without a Rule binding.
//!
//! This test pins the help text of each CLI command, so a change to a flag name shows
//! here first. A changed name changes its pin in the same commit. The MCP input schema
//! pins are in `provenance-mcp/tests/surface_names.rs`.

#[allow(dead_code)]
mod support {
    pub mod surface;
}

use std::path::PathBuf;
use support::surface::help;

/// The argument lists whose help each pin holds, by pin name.
const HELP: &[(&str, &[&str])] = &[
    // The dev-build-only dogfood group shows in the root help of a dogfood build.
    (
        if cfg!(feature = "dogfood") {
            "root_dogfood"
        } else {
            "root"
        },
        &[],
    ),
    ("get", &["record_x", "get"]),
    ("trace", &["record_x", "trace"]),
    ("create", &["record_x", "create"]),
    ("create_rule", &["record_x", "create", "--type", "rule"]),
    ("update", &["record_x", "update"]),
    ("answer", &["record_x", "answer"]),
    ("claim", &["record_x", "claim"]),
    ("release", &["record_x", "release"]),
    ("submit", &["record_x", "submit"]),
    ("discussions", &["record_x", "discussions"]),
    ("discussion", &["discussion_x", "discussion"]),
    ("discuss", &["record_x", "discuss"]),
    ("reply", &["discussion_x", "reply"]),
    ("search", &["search"]),
    ("api", &["api"]),
    ("check", &["check"]),
    ("prime", &["prime"]),
    ("init", &["init"]),
    ("cargo_init", &["__cargo-init"]),
    ("materialize", &["materialize"]),
    ("export", &["export"]),
    ("import", &["import"]),
    ("merge_jsonl", &["merge-jsonl"]),
    ("validate", &["validate"]),
    ("schema_show", &["schema", "show"]),
    ("dictionary_import", &["dictionary", "import"]),
    ("dictionary_status", &["dictionary", "status"]),
    ("graph_reference_issue", &["graph-reference", "issue"]),
    ("graph_reference_show", &["graph-reference", "show"]),
    ("graph_reference_verify", &["graph-reference", "verify"]),
    (
        "graph_reference_exact_export",
        &["graph-reference", "exact-export"],
    ),
    ("swarm_backtrace_land", &["swarm-backtrace", "land"]),
    ("skills_list", &["skills", "list"]),
    ("skills_show", &["skills", "show"]),
    ("skills_install", &["skills", "install"]),
    ("report_render", &["report", "render"]),
    ("report_build", &["report", "build"]),
    ("review", &["review"]),
    ("docs_serve", &["docs", "serve"]),
    ("mcp", &["mcp"]),
];

fn pin_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/surface_names")
        .join(name)
}

/// Compares each actual text with its pin. A missing or different pin prints the actual
/// text between marker lines, so a reviewer can read the change, and fails.
fn assert_pins(actual: &[(String, String)]) {
    let mut changed = Vec::new();
    for (name, text) in actual {
        let expected = std::fs::read_to_string(pin_path(name)).ok();
        if expected.as_deref() != Some(text.as_str()) {
            println!("=== PIN {name}\n{text}=== END PIN {name}");
            changed.push(name.clone());
        }
    }
    assert!(changed.is_empty(), "changed pins: {changed:?}");
}

/// The help text with the executable suffix removed from the binary name, so the
/// `Usage: provenance.exe` of a Windows build matches the pin.
fn without_suffix(text: String) -> String {
    match std::env::consts::EXE_SUFFIX {
        "" => text,
        suffix => text.replace(&format!("provenance{suffix}"), "provenance"),
    }
}

#[test]
fn cli_help_of_each_command_is_pinned() {
    let actual = HELP
        .iter()
        .map(|(name, words)| (format!("cli_{name}.txt"), without_suffix(help(words))))
        .collect::<Vec<_>>();
    assert_pins(&actual);
}
