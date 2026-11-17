//! The sites that `check --bindings` reports for one source file in a new
//! repository.
use assert_cmd::Command;
use serde_json::Value;

/// The scan report of a new repository that holds `source` at `path`.
pub fn scan(path: &str, source: &str) -> Value {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path().to_str().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            root,
            "--scope",
            "demo",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    let file = repo.path().join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, source).unwrap();
    let output = repo.path().join("report.json");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--bindings",
            "--repo",
            root,
            "--path",
            root,
            "--scope",
            "demo",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap()
}

/// A site of a scan report: its line, its Rule id, its verification method
/// and its item name. No method is an implementation site.
pub type Site = (u64, String, Option<String>, Option<String>);

/// The sites of a scan report, in line order.
pub fn sites(report: &Value) -> Vec<Site> {
    let text = |site: &Value, key: &str| site[key].as_str().map(str::to_owned);
    let mut sites = ["annotations", "bindings"]
        .iter()
        .flat_map(|key| report[key].as_array().unwrap())
        .map(|site| {
            (
                site["line"].as_u64().unwrap(),
                text(site, "rule_id").unwrap(),
                text(site, "verification"),
                text(site, "function_name").or_else(|| text(site, "item_name")),
            )
        })
        .collect::<Vec<_>>();
    sites.sort();
    sites
}

/// The Rule id and the item name of each site in a scan report, in line order.
pub fn item_names(report: &Value) -> Vec<(String, Option<String>)> {
    sites(report)
        .into_iter()
        .map(|(_, rule, _, name)| (rule, name))
        .collect()
}

/// The line, the Rule id and the verification method of each site in a scan
/// report, in line order.
pub fn places(report: &Value) -> Vec<(u64, String, Option<String>)> {
    sites(report)
        .into_iter()
        .map(|(line, rule, method, _)| (line, rule, method))
        .collect()
}

pub fn place(line: u64, rule: &str, method: Option<&str>) -> (u64, String, Option<String>) {
    (line, rule.to_owned(), method.map(str::to_owned))
}

pub fn named(rule: &str, name: &str) -> (String, Option<String>) {
    (rule.to_owned(), Some(name.to_owned()))
}
