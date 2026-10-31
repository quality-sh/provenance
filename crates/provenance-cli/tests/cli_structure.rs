use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const RUST_FILE_LINE_LIMIT: usize = 500;

/// Cap on the `///` block sitting directly above a `#[rule("...")]` attribute.
///
/// Set to the largest header standing today, so no rule header may grow and no
/// new one may be born larger than the widest one already earned. The rules
/// spending the budget, measured over the tracked tree:
///
/// - 29 `rule_reference_verified` (`provenance-store/src/graph_reference.rs`)
/// - 27 `rule_reads_supported_version_only` (`state_store/readers.rs`)
/// - 26 `rule_disposition_write_gate` (`state_store/proposal_writers.rs`)
/// - 21 `rule_docs_links_resolve` (`provenance-cli/src/docs.rs`)
/// - 16 `rule_generated_outputs_ignored` (`provenance-cli/src/gitignore.rs`)
///
/// Shrinking one of those is the way to lower this number; nothing else needs
/// to move.
const RULE_DOC_LINE_LIMIT: usize = 29;

/// Phrases that are record-keeping by definition, whatever they are wrapped in.
///
/// Kept literal and short on purpose: this audit judges shape, not meaning.
const BANNED_RULE_DOC_PHRASES: [&str; 2] = ["Amended 20", "tracked in beads"];

#[test]
fn tracked_rust_files_stay_below_the_hard_line_limit() {
    let workspace = workspace_root();
    for relative_path in tracked_rust_files(&workspace) {
        let source = fs::read_to_string(workspace.join(&relative_path)).expect("read tracked file");
        let line_count = source.lines().count();
        assert!(
            line_count < RUST_FILE_LINE_LIMIT,
            "{relative_path} has {line_count} lines; tracked Rust files must stay below {RUST_FILE_LINE_LIMIT}",
        );
    }
}

#[test]
fn rule_doc_headers_stay_within_their_budget() {
    let workspace = workspace_root();
    let mut rules_seen = 0usize;
    for relative_path in tracked_rust_files(&workspace) {
        let source = fs::read_to_string(workspace.join(&relative_path)).expect("read tracked file");
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            let Some(rule_id) = rule_id_on_line(line) else {
                continue;
            };
            rules_seen += 1;
            let doc_block = doc_block_above(&lines, index);
            assert!(
                doc_block.len() <= RULE_DOC_LINE_LIMIT,
                "{relative_path}: the doc header on {rule_id} is {} lines; the cap is {RULE_DOC_LINE_LIMIT}. \
                 Elaboration lives in the rule's graph record; constraints the code cannot show may stay.",
                doc_block.len(),
            );
            let header = doc_block.join("\n");
            for phrase in BANNED_RULE_DOC_PHRASES {
                assert!(
                    !header.contains(phrase),
                    "{relative_path}: the doc header on {rule_id} contains \"{phrase}\". \
                     Elaboration lives in the rule's graph record; constraints the code cannot show may stay.",
                );
            }
        }
    }
    assert!(rules_seen > 0, "rule doc audit found no #[rule(\"...\")]");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate is nested under the workspace root")
        .to_path_buf()
}

fn tracked_rust_files(workspace: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args([
            "-C",
            workspace.to_str().expect("workspace path is UTF-8"),
            "ls-files",
            "-z",
            "--",
            ":(glob)**/*.rs",
        ])
        .output()
        .expect("run git ls-files for the tracked Rust audits");
    assert!(output.status.success(), "git ls-files failed");
    let files = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8(path.to_vec()).expect("tracked Rust path is UTF-8"))
        .collect::<Vec<_>>();
    assert!(!files.is_empty(), "tracked Rust audit found no files");
    files
}

/// Reads the rule id off a line that is itself a `#[rule("...")]` attribute.
///
/// Mentions of the attribute inside a string literal or a doc comment carry an
/// escaped or prefixed opening, so matching the exact attribute text leaves the
/// scanner's own fixtures and the macro crate's examples alone.
fn rule_id_on_line(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("#[rule(\"")?;
    rest.split('"').next()
}

/// Collects the `///` block belonging to the attribute at `attribute_index`.
///
/// Other attributes may sit between the doc comment and `#[rule]`, so those are
/// stepped over first; the block is then the run of doc lines directly above.
fn doc_block_above(lines: &[&str], attribute_index: usize) -> Vec<String> {
    let mut cursor = attribute_index;
    while cursor > 0 && lines[cursor - 1].trim_start().starts_with("#[") {
        cursor -= 1;
    }
    let mut block = Vec::new();
    while cursor > 0 && lines[cursor - 1].trim_start().starts_with("///") {
        cursor -= 1;
        block.push(lines[cursor].trim_start().to_owned());
    }
    block.reverse();
    block
}
