//! Compares the direct dependencies of each workspace crate in `cargo metadata`
//! with the crate dependency policy in `.config/crate-dependencies.json`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use provenance_macros::{rule, verifies};
use serde::Deserialize;
use serde_json::{json, Value};

const POLICY_FILE: &str = ".config/crate-dependencies.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    host_crates: BTreeSet<String>,
    crates: BTreeMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Entry {
    dependencies: BTreeSet<String>,
    dev_dependencies: BTreeSet<String>,
}

impl Entry {
    fn list(&self, list: &str) -> &BTreeSet<String> {
        if list == "dev-dependencies" {
            &self.dev_dependencies
        } else {
            &self.dependencies
        }
    }
}

/// One direct edge from a workspace crate, by package name and not by rename.
struct Edge<'a> {
    dependency: &'a str,
    kind: &'a str,
}

impl Edge<'_> {
    /// Normal and build edges go in `dependencies`, dev edges in `dev-dependencies`.
    fn list(&self) -> &'static str {
        if self.kind == "dev" {
            "dev-dependencies"
        } else {
            "dependencies"
        }
    }
}

fn edges(package: &Value) -> impl Iterator<Item = Edge<'_>> {
    package["dependencies"]
        .as_array()
        .expect("package dependencies are an array")
        .iter()
        .map(|dependency| Edge {
            dependency: dependency["name"]
                .as_str()
                .expect("dependency name is text"),
            kind: dependency["kind"].as_str().unwrap_or("normal"),
        })
}

/// Returns each difference between the policy and the metadata. Only workspace
/// crates and host crates are checked; other outside crates are ignored.
#[rule("rule_workspace_crate_dependencies_match_policy")]
fn dependency_refusals(policy: &Policy, metadata: &Value) -> Vec<String> {
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata packages are an array");
    let members: BTreeSet<&str> = packages
        .iter()
        .map(|package| package["name"].as_str().expect("package name is text"))
        .collect();
    let checked = |name: &str| members.contains(name) || policy.host_crates.contains(name);
    let mut refusals = BTreeSet::new();

    for name in policy.crates.keys() {
        if !members.contains(name.as_str()) {
            refusals.insert(format!(
                "{POLICY_FILE} has an entry for {name}, which is not a workspace crate"
            ));
        }
    }
    for package in packages {
        let name = package["name"].as_str().expect("package name is text");
        let Some(entry) = policy.crates.get(name) else {
            refusals.insert(format!("{name} has no entry in {POLICY_FILE}"));
            continue;
        };
        let mut found = BTreeSet::new();
        for edge in edges(package).filter(|edge| checked(edge.dependency)) {
            found.insert((edge.list(), edge.dependency));
            if !entry.list(edge.list()).contains(edge.dependency) {
                refusals.insert(format!(
                    "{name} has a {} dependency on {} that {POLICY_FILE} does not allow",
                    edge.kind, edge.dependency
                ));
            }
        }
        for list in ["dependencies", "dev-dependencies"] {
            for dependency in entry.list(list) {
                if !found.contains(&(list, dependency.as_str())) {
                    refusals.insert(format!(
                        "{POLICY_FILE} allows {name} to have {dependency} in {list}, \
                         but that edge does not exist"
                    ));
                }
            }
        }
    }
    refusals.into_iter().collect()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate is in the workspace crates directory")
        .to_owned()
}

fn workspace_metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root())
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata")
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", conformance)]
fn workspace_crates_depend_only_on_what_the_policy_allows() {
    let policy = fs::read_to_string(workspace_root().join(POLICY_FILE)).expect("read policy");
    let policy: Policy = serde_json::from_str(&policy).expect("parse policy");

    let refusals = dependency_refusals(&policy, &workspace_metadata());

    assert!(refusals.is_empty(), "{}", refusals.join("\n"));
}

fn fixture_policy() -> Value {
    json!({
        "host_crates": ["fs2", "rustix", "tempfile", "tokio", "windows-sys"],
        "crates": {
            "provenance-macros": { "dependencies": [], "dev-dependencies": [] },
            "provenance-core": {
                "dependencies": ["provenance-macros"],
                "dev-dependencies": []
            },
            "provenance-store": {
                "dependencies": ["fs2", "provenance-core", "rustix", "tokio"],
                "dev-dependencies": ["tempfile"]
            },
            "provenance-report": {
                "dependencies": ["provenance-core", "provenance-store"],
                "dev-dependencies": []
            }
        }
    })
}

fn fixture_metadata() -> Value {
    serde_json::from_str(include_str!("fixtures/crate_dependencies/metadata.json"))
        .expect("parse fixture metadata")
}

fn add_edge(metadata: &mut Value, package: &str, edge: Value) {
    let package = metadata["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|candidate| candidate["name"] == package)
        .unwrap();
    package["dependencies"].as_array_mut().unwrap().push(edge);
}

fn refusals(policy: Value, metadata: Value) -> Vec<String> {
    dependency_refusals(&serde_json::from_value(policy).unwrap(), &metadata)
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn fixture_workspace_matches_its_policy() {
    assert_eq!(
        refusals(fixture_policy(), fixture_metadata()),
        Vec::<String>::new()
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn new_host_edge_from_core_is_refused() {
    let mut metadata = fixture_metadata();
    add_edge(
        &mut metadata,
        "provenance-core",
        json!({ "name": "tokio", "rename": null, "kind": null, "target": null }),
    );

    assert_eq!(
        refusals(fixture_policy(), metadata),
        ["provenance-core has a normal dependency on tokio that \
          .config/crate-dependencies.json does not allow"]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn new_workspace_edge_from_store_is_refused() {
    let mut metadata = fixture_metadata();
    add_edge(
        &mut metadata,
        "provenance-store",
        json!({ "name": "provenance-report", "rename": null, "kind": null, "target": null }),
    );

    assert_eq!(
        refusals(fixture_policy(), metadata),
        [
            "provenance-store has a normal dependency on provenance-report that \
          .config/crate-dependencies.json does not allow"
        ]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn renamed_edge_is_refused_by_its_package_name() {
    let mut metadata = fixture_metadata();
    add_edge(
        &mut metadata,
        "provenance-report",
        json!({ "name": "tokio", "rename": "rt", "kind": null, "target": null }),
    );

    assert_eq!(
        refusals(fixture_policy(), metadata),
        ["provenance-report has a normal dependency on tokio that \
          .config/crate-dependencies.json does not allow"]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn target_specific_host_edge_is_refused() {
    let mut metadata = fixture_metadata();
    add_edge(
        &mut metadata,
        "provenance-report",
        json!({ "name": "windows-sys", "rename": null, "kind": "build", "target": "cfg(windows)" }),
    );

    assert_eq!(
        refusals(fixture_policy(), metadata),
        [
            "provenance-report has a build dependency on windows-sys that \
          .config/crate-dependencies.json does not allow"
        ]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn dev_edge_allowed_only_in_dependencies_is_refused() {
    let mut policy = fixture_policy();
    policy["crates"]["provenance-store"] = json!({
        "dependencies": ["fs2", "provenance-core", "rustix", "tempfile", "tokio"],
        "dev-dependencies": []
    });

    assert_eq!(
        refusals(policy, fixture_metadata()),
        [
            ".config/crate-dependencies.json allows provenance-store to have tempfile in \
             dependencies, but that edge does not exist",
            "provenance-store has a dev dependency on tempfile that \
             .config/crate-dependencies.json does not allow",
        ]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn member_without_an_entry_is_refused() {
    let mut policy = fixture_policy();
    policy["crates"]
        .as_object_mut()
        .unwrap()
        .remove("provenance-report");

    assert_eq!(
        refusals(policy, fixture_metadata()),
        ["provenance-report has no entry in .config/crate-dependencies.json"]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn entry_for_a_crate_outside_the_workspace_is_refused() {
    let mut policy = fixture_policy();
    policy["crates"]["provenance-contract"] = json!({ "dependencies": [], "dev-dependencies": [] });

    assert_eq!(
        refusals(policy, fixture_metadata()),
        [
            ".config/crate-dependencies.json has an entry for provenance-contract, \
          which is not a workspace crate"
        ]
    );
}

#[test]
#[verifies("rule_workspace_crate_dependencies_match_policy", examples)]
fn allowed_edge_missing_from_metadata_is_refused() {
    let mut policy = fixture_policy();
    policy["crates"]["provenance-core"]["dependencies"] = json!(["provenance-macros", "tokio"]);

    assert_eq!(
        refusals(policy, fixture_metadata()),
        [
            ".config/crate-dependencies.json allows provenance-core to have tokio in \
          dependencies, but that edge does not exist"
        ]
    );
}
