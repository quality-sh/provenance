use assert_cmd::Command;
use serde_json::Value;

pub struct Fixture {
    _temp: tempfile::TempDir,
    pub repo: std::path::PathBuf,
    pub source: std::path::PathBuf,
    pub baseline: std::path::PathBuf,
}

impl Fixture {
    pub fn new(source: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().to_path_buf();
        let source_dir = repo.join("src");
        let source_path = source_dir.join("rules.rs");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(&source_path, source).unwrap();

        provenance(&[
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ]);
        provenance(&[
            "req_anchor",
            "create",
            "--type",
            "requirement",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--statement",
            "The anchor requirement holds",
        ]);
        provenance(&[
            "rule_anchor",
            "create",
            "--type",
            "rule",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--requirement-id",
            "req_anchor",
            "--statement",
            "Anchor the primary implementation",
            "--status",
            "active",
            "--severity",
            "high",
        ]);

        let fixture = Self {
            baseline: repo.join("baseline.json"),
            source: source_path,
            repo,
            _temp: temp,
        };
        let baseline = fixture.scan(None, false);
        std::fs::write(
            &fixture.baseline,
            serde_json::to_vec_pretty(&baseline).unwrap(),
        )
        .unwrap();
        fixture
    }

    pub fn scan(&self, baseline: Option<&std::path::Path>, validate_rules: bool) -> Value {
        self.scan_at(self.source.parent().unwrap(), baseline, validate_rules)
    }

    /// The scan report of `check --bindings` at `path`. The check always validates
    /// Rules, so `_validate_rules` no longer changes the report.
    pub fn scan_at(
        &self,
        path: &std::path::Path,
        baseline: Option<&std::path::Path>,
        _validate_rules: bool,
    ) -> Value {
        serde_json::from_slice(&self.report(path, baseline)).unwrap()
    }

    /// Runs `check --bindings --scope default` and returns its report file.
    fn report(&self, path: &std::path::Path, baseline: Option<&std::path::Path>) -> Vec<u8> {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("report.json");
        let mut command = Command::cargo_bin("provenance").unwrap();
        command.args([
            "check",
            "--bindings",
            "--repo",
            self.repo.to_str().unwrap(),
            "--path",
            path.to_str().unwrap(),
            "--scope",
            "default",
            "--output",
            output.to_str().unwrap(),
        ]);
        if let Some(baseline) = baseline {
            command.args(["--baseline", baseline.to_str().unwrap()]);
        }
        command.assert().success();
        std::fs::read(output).unwrap()
    }

    pub fn rescan(&self) -> Value {
        self.scan(Some(&self.baseline), true)
    }

    pub fn scan_bytes(&self, baseline: &std::path::Path) -> Vec<u8> {
        self.report(self.source.parent().unwrap(), Some(baseline))
    }
}

fn provenance(args: &[&str]) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
        .success();
}

pub fn binding<'a>(report: &'a Value, item: &str, state: &str) -> &'a Value {
    report["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|binding| {
            binding["item_name"] == item && binding["anchor_state"].as_str() == Some(state)
        })
        .unwrap_or_else(|| panic!("missing {state} anchor for {item}: {report:#}"))
}

pub fn annotation<'a>(report: &'a Value, function: &str, state: &str) -> &'a Value {
    report["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|annotation| {
            annotation["function_name"] == function
                && annotation["anchor_state"].as_str() == Some(state)
        })
        .unwrap_or_else(|| panic!("missing {state} anchor for {function}: {report:#}"))
}

pub const ORIGINAL: &str = "#[rule(\"rule_anchor\")]\n\
fn decide_anchor() {}\n\n\
#[verifies(\"rule_anchor\", examples)]\n\
fn verifies_anchor() {}\n";

pub const DUPLICATES: &str =
    "mod first {\n#[verifies(\"rule_anchor\", examples)]\nfn duplicate() {}\n}\n\n\
     mod second {\n#[verifies(\"rule_anchor\", examples)]\nfn duplicate() {}\n}\n";
