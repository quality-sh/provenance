use crate::review_paths::provenance;
pub use crate::stale_repo::{commit, git, init_repo, remove_object, write};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The two commits of the fixture range and the repository that holds them.
pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub base: String,
    pub head: String,
}

impl Fixture {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Replaces the commit ids, which change with each fixture, by names.
    pub fn normalized(&self, value: &Value) -> Value {
        let text = value
            .to_string()
            .replace(&self.base, "BASE")
            .replace(&self.head, "HEAD");
        serde_json::from_str(&text).unwrap()
    }
}

const RULES: [&str; 9] = [
    "rule_kept",
    "rule_touched",
    "rule_shifted",
    "rule_crossed",
    "rule_removed",
    "rule_twins",
    "rule_renamed",
    "rule_typed",
    "rule_tested",
];

/// Applies one Rule for each id. The Rule `rule_typed` is bound to the
/// function `startWorkflow` in `src/runtime.ts`.
pub fn apply_rules(repo: &Path, ids: &[&str]) {
    let rules = ids
        .iter()
        .map(|id| {
            let mut rule = json!({
                "key": id,
                "id": id,
                "requirement": "workflows",
                "statement": "Accepted workflows start"
            });
            if *id == "rule_typed" {
                rule["implementation"] =
                    json!({"file": "src/runtime.ts", "symbol": "startWorkflow"});
            }
            rule
        })
        .collect::<Vec<_>>();
    crate::stale_repo::apply_spec(
        repo,
        &json!({
            "spec": "runtime",
            "declared_by": "spec://typescript/runtime",
            "requirements": [{"key": "workflows", "statement": "Accepted workflows execute"}],
            "rules": rules
        }),
    );
}

pub fn function(rule: &str, name: &str, body: &str) -> String {
    format!("#[rule(\"{rule}\")]\nfn {name}() -> u32 {{\n    {body}\n}}\n")
}

/// A base and a head commit whose diff gives each site state: touched,
/// moved in one file, moved to another file, gone, an ambiguous pair, a
/// renamed file, a typed binding, a verification and a marker for a Rule
/// that the graph does not hold.
pub fn fixture() -> Fixture {
    let directory = init_repo();
    let repo = directory.path();
    let padding = (0..40)
        .map(|line| format!("const PAD_{line}: u32 = {line};\n"))
        .collect::<Vec<_>>()
        .concat();
    write_base(repo, &padding);
    apply_rules(repo, &RULES);
    let base = commit(repo, "base");
    write_head(repo, &padding);
    let head = commit(repo, "head");
    Fixture {
        directory,
        base,
        head,
    }
}

fn write_base(repo: &Path, padding: &str) {
    write(repo, "src/kept.rs", function("rule_kept", "kept", "1"));
    write(
        repo,
        "src/touched.rs",
        function("rule_touched", "touched", "1"),
    );
    write(
        repo,
        "src/shifted.rs",
        function("rule_shifted", "shifted", "1"),
    );
    write(
        repo,
        "src/from.rs",
        format!("{}{padding}", function("rule_crossed", "crossed", "7")),
    );
    write(
        repo,
        "src/removed.rs",
        format!(
            "{}fn stays() {{}}\n",
            function("rule_removed", "removed", "1")
        ),
    );
    let twin = function("rule_twins", "twin", "2");
    write(repo, "src/twins.rs", format!("{twin}\n{twin}"));
    write(
        repo,
        "src/renamed_before.rs",
        function("rule_renamed", "renamed", "3"),
    );
    write(
        repo,
        "src/unknown.rs",
        function("rule_not_in_graph", "unknown", "1"),
    );
    write(
        repo,
        "src/runtime.ts",
        "export function startWorkflow() { return 'started'; }\n",
    );
    write(
        repo,
        "tests/checks.rs",
        "#[verifies(\"rule_tested\", examples)]\nfn checks() {\n    assert_eq!(1, 1);\n}\n",
    );
    write(repo, "notes.md", "Notes.\n");
}

fn write_head(repo: &Path, padding: &str) {
    write(
        repo,
        "src/touched.rs",
        function("rule_touched", "touched", "2"),
    );
    write(
        repo,
        "src/shifted.rs",
        format!("{padding}{}", function("rule_shifted", "shifted", "1")),
    );
    write(repo, "src/from.rs", padding);
    write(
        repo,
        "src/to.rs",
        format!(
            "fn first() {{}}\n{}",
            function("rule_crossed", "crossed", "7")
        ),
    );
    write(repo, "src/removed.rs", "fn stays() {}\n");
    write(repo, "src/twins.rs", function("rule_twins", "twin", "2"));
    git(
        repo,
        &["mv", "src/renamed_before.rs", "src/renamed_after.rs"],
    );
    write(
        repo,
        "src/unknown.rs",
        function("rule_not_in_graph", "unknown", "2"),
    );
    write(
        repo,
        "src/runtime.ts",
        "export function startWorkflow() { return 'running'; }\n",
    );
    write(
        repo,
        "tests/checks.rs",
        "#[verifies(\"rule_tested\", examples)]\nfn checks() {\n    assert_eq!(2, 2);\n}\n",
    );
    write(repo, "notes.md", "Notes, changed.\n");
}

/// The CLI answer of `api <path>`: exit code, the JSON on standard output
/// and the text on standard error.
pub fn cli_api(repo: &Path, path: &str, query: &[(&str, &str)]) -> (i32, Value, String) {
    let mut command = provenance();
    command.args(["api", path]);
    for (name, value) in query {
        command.arg("--query").arg(format!("{name}={value}"));
    }
    let output = command
        .arg("--repo")
        .arg(repo)
        .args(["--scope", "default", "--format", "json"])
        .output()
        .unwrap();
    let stdout = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    (
        output.status.code().unwrap_or(-1),
        stdout,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The CLI answer of the stale query.
pub fn cli_stale(repo: &Path, query: &[(&str, &str)]) -> (i32, Value, String) {
    let mut all = vec![("query", "stale")];
    all.extend_from_slice(query);
    cli_api(repo, "rules", &all)
}

/// The JSON error object of a CLI refusal on standard error.
pub fn cli_error(stderr: &str) -> Value {
    let text = stderr.trim();
    let text = text.strip_prefix("Error: ").unwrap_or(text);
    serde_json::from_str::<Value>(text).unwrap_or_else(|error| panic!("{error}: {stderr}"))["error"]
        .clone()
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stale_answers")
        .join(format!("{name}.json"))
}

/// Compares answers with their recorded files. Each missing or different
/// file prints the answer, so a new record can be taken from the test
/// output; the check fails once, after all answers.
#[derive(Default)]
pub struct Goldens {
    different: Vec<String>,
}

impl Goldens {
    pub fn check(&mut self, name: &str, actual: &Value) {
        let expected = std::fs::read_to_string(golden_path(name))
            .ok()
            .map(|text| serde_json::from_str::<Value>(&text).unwrap());
        if expected.as_ref() != Some(actual) {
            println!("GOLDEN {name} {actual}");
            self.different.push(name.to_string());
        }
    }

    pub fn finish(self) {
        assert!(
            self.different.is_empty(),
            "answers that are not the recorded answers: {:?}",
            self.different
        );
    }
}
