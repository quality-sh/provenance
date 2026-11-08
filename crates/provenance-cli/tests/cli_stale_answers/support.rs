use assert_cmd::Command;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

pub fn git(repo: &Path, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub fn write(repo: &Path, relative: &str, contents: impl AsRef<[u8]>) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

pub fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["add", "-A", "."]);
    git(repo, &["commit", "-q", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

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

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    git(directory.path(), &["init", "-q"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(directory.path(), &["config", "user.name", "Test"]);
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    directory
}

fn apply_rules(repo: &Path) {
    let rules = RULES
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
    let input = json!({
        "spec": "runtime",
        "declared_by": "spec://typescript/runtime",
        "requirements": [{"key": "workflows", "statement": "Accepted workflows execute"}],
        "rules": rules
    });
    provenance()
        .args([
            "api",
            "authoring-changes",
            "--method",
            "post",
            "--input",
            "-",
        ])
        .arg("--repo")
        .arg(repo)
        .args(["--scope", "default", "--format", "json"])
        .write_stdin(serde_json::to_vec(&input).unwrap())
        .assert()
        .success();
}

fn function(rule: &str, name: &str, body: &str) -> String {
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
    apply_rules(repo);
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

/// The CLI answer: exit code, the JSON on standard output and the text on
/// standard error.
pub fn cli_stale(repo: &Path, query: &[(&str, &str)]) -> (i32, Value, String) {
    let mut command = provenance();
    command.args(["api", "rules", "--query", "query=stale"]);
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
