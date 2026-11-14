use serde_json::Value;
use std::{
    collections::BTreeSet,
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
/// - 26 `rule_disposition_write_gate` (`state_store/proposal_writers.rs`)
/// - 21 `rule_docs_links_resolve` (`provenance-transport/src/docs.rs`)
/// - 16 `rule_generated_outputs_ignored` (`provenance-transport/src/gitignore.rs`)
///
/// Shrinking one of those is the way to lower this number; nothing else needs
/// to move.
const RULE_DOC_LINE_LIMIT: usize = 26;

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

/// Checks that Cargo compiles each test file in the `tests/` directory of a crate.
///
/// A crate with `autotests = false` lists its test targets. A group target
/// `tests/<group>/main.rs` loads test files of `tests/` as modules. A new test
/// file that no target loads is not compiled, so its tests do not run. This is
/// an aid for the test layout. No Rule states the layout.
#[test]
fn each_test_file_is_in_a_test_target() {
    let workspace = workspace_root();
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&workspace)
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("parse cargo metadata");
    let mut files_seen = 0usize;
    for package in metadata["packages"].as_array().expect("metadata packages") {
        let manifest = PathBuf::from(package["manifest_path"].as_str().expect("manifest path"));
        let tests = manifest.parent().expect("crate directory").join("tests");
        let Ok(entries) = fs::read_dir(&tests) else {
            continue;
        };
        let targets = package["targets"]
            .as_array()
            .expect("package targets")
            .iter()
            .filter(|target| target["kind"][0] == "test")
            .map(|target| PathBuf::from(target["src_path"].as_str().expect("target path")))
            .collect::<BTreeSet<_>>();
        let mut loaded = BTreeSet::new();
        let mut texts = Vec::new();
        for entry in entries {
            let path = entry.expect("read tests entry").path();
            let group_main = path.join("main.rs");
            if group_main.is_file() {
                assert!(
                    targets.contains(&group_main),
                    "{} is not a test target of {}; add it as a [[test]]",
                    group_main.display(),
                    package["name"],
                );
                let text = fs::read_to_string(&group_main).expect("read group main.rs");
                for line in text.lines() {
                    if let Some(file) = line.trim().strip_prefix("#[path = \"../") {
                        loaded.insert(tests.join(file.trim_end_matches("\"]")));
                    }
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                texts.push((
                    path.clone(),
                    fs::read_to_string(&path).expect("read test file"),
                ));
            }
        }
        for (path, _) in &texts {
            files_seen += 1;
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("file stem");
            let support = texts.iter().any(|(other, text)| {
                other != path
                    && (text.contains(&format!("mod {stem};"))
                        || text.contains(&format!("\"{stem}.rs\"")))
            });
            assert!(
                targets.contains(path) || loaded.contains(path) || support,
                "{} is in no test target of {}: add it to a group main.rs or as a [[test]]",
                path.display(),
                package["name"],
            );
        }
    }
    assert!(files_seen > 0, "the test layout check found no test files");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("CLI crate is nested under the workspace root")
        .to_path_buf()
}

fn tracked_rust_files(workspace: &Path) -> Vec<String> {
    let output = provenance_store::git_process::git()
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
