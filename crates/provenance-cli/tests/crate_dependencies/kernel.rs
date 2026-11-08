//! Checks that the default features of each kernel crate reach no host crate.
//! Only the host feature of a kernel crate turns on its host dependencies.

use std::collections::BTreeSet;

use provenance_macros::{rule, verifies};
use serde_json::{json, Value};

use super::{
    edges, fixture_metadata, fixture_policy, read_policy, workspace_metadata, Edge, Policy,
    POLICY_FILE,
};

fn is_proc_macro(package: &Value) -> bool {
    package["targets"].as_array().is_some_and(|targets| {
        targets.iter().any(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "proc-macro"))
        })
    })
}

/// Each item of the features table that `start` reaches, `start` included.
fn reached(package: &Value, start: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut next = vec![start.to_owned()];
    while let Some(item) = next.pop() {
        if let Some(items) = package["features"][&item].as_array() {
            next.extend(items.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        seen.insert(item);
    }
    seen
}

/// The name that the features table uses for the edge.
fn key<'a>(edge: &Edge<'a>) -> &'a str {
    edge.spec["rename"].as_str().unwrap_or(edge.dependency)
}

/// Tells if one of the feature items turns on the optional edge.
fn turns_on(items: &BTreeSet<String>, edge: &Edge) -> bool {
    let key = key(edge);
    items.iter().any(|item| {
        item == key || *item == format!("dep:{key}") || item.starts_with(&format!("{key}/"))
    })
}

fn optional(edge: &Edge) -> bool {
    edge.spec["optional"].as_bool().unwrap_or(false)
}

fn names_feature(edge: &Edge, feature: &str) -> bool {
    edge.spec["features"]
        .as_array()
        .is_some_and(|features| features.iter().any(|item| item == feature))
}

/// Returns each kernel crate whose policy rows or features let its default
/// features reach a host crate.
#[rule("rule_kernel_default_features_have_no_host_crate")]
fn kernel_refusals(policy: &Policy, metadata: &Value) -> Vec<String> {
    let packages = metadata["packages"].as_array().expect("packages");
    let kernel = &policy.kernel;
    let host = kernel.host_feature.as_str();
    let mut refusals = Vec::new();
    if !kernel.target.starts_with("wasm32-") {
        refusals.push(format!(
            "the kernel target {} in {POLICY_FILE} is not a wasm32 target",
            kernel.target
        ));
    }
    for (name, entry) in &policy.crates {
        if !kernel.crates.contains(name) && !entry.host_dependencies.is_empty() {
            refusals.push(format!(
                "{POLICY_FILE} gives {name} host-dependencies, but {name} is not a kernel crate"
            ));
        }
    }
    for name in &kernel.crates {
        let Some(package) = packages.iter().find(|package| package["name"] == *name) else {
            refusals.push(format!("kernel crate {name} is not a workspace crate"));
            continue;
        };
        let row = policy.forbidden.get(name);
        let outside = packages
            .iter()
            .filter(|other| !is_proc_macro(other))
            .filter_map(|other| other["name"].as_str())
            .filter(|other| !kernel.crates.contains(*other));
        for crate_name in policy.host_crates.iter().map(String::as_str).chain(outside) {
            if !row.is_some_and(|row| row.contains(crate_name)) {
                refusals.push(format!(
                    "the forbidden row for kernel crate {name} in {POLICY_FILE} does not name \
                     {crate_name}"
                ));
            }
        }
        let default = reached(package, "default");
        let on = reached(package, host);
        if default.contains(host) {
            refusals.push(format!("the default feature of {name} turns on {host}"));
        }
        let host_dependencies = policy
            .crates
            .get(name)
            .map(|entry| &entry.host_dependencies);
        for edge in edges(package).filter(|edge| edge.list() == "dependencies") {
            let in_default = !optional(&edge) || turns_on(&default, &edge);
            let host_in_dependency = format!("{}/{host}", key(&edge));
            if (in_default && names_feature(&edge, host)) || default.contains(&host_in_dependency) {
                refusals.push(format!(
                    "the default feature of {name} turns on {host} in {}",
                    edge.dependency
                ));
            }
            if !host_dependencies.is_some_and(|list| list.contains(edge.dependency)) {
                continue;
            }
            if !optional(&edge) {
                refusals.push(format!(
                    "{name} has a {} dependency on {} in host-dependencies that is not optional",
                    edge.kind, edge.dependency
                ));
            } else if turns_on(&default, &edge) {
                refusals.push(format!(
                    "the default feature of {name} turns on its host dependency {}",
                    edge.dependency
                ));
            } else if !turns_on(&on, &edge) {
                refusals.push(format!(
                    "the {host} feature of {name} does not turn on its host dependency {}",
                    edge.dependency
                ));
            }
        }
    }
    refusals
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", conformance)]
fn kernel_default_features_reach_no_host_crate() {
    let refusals = kernel_refusals(&read_policy(), &workspace_metadata());

    assert!(refusals.is_empty(), "{}", refusals.join("\n"));
}

fn kernel(policy: Value, metadata: &Value) -> Vec<String> {
    kernel_refusals(&serde_json::from_value(policy).unwrap(), metadata)
}

fn set_features(metadata: &mut Value, name: &str, features: Value) {
    let packages = metadata["packages"].as_array_mut().unwrap();
    let package = packages.iter_mut().find(|package| package["name"] == name);
    package.unwrap()["features"] = features;
}

/// The fixture policy with the store as a kernel crate that has host dependencies.
fn store_kernel_policy(host_dependencies: &Value) -> Value {
    let mut policy = fixture_policy();
    policy["kernel"]["crates"] = json!(["provenance-core", "provenance-store"]);
    policy["forbidden"]["provenance-store"] = json!([
        "fs2",
        "provenance-cli",
        "provenance-mcp",
        "provenance-report",
        "rustix",
        "tempfile",
        "tokio",
        "windows-sys"
    ]);
    policy["crates"]["provenance-store"] = json!({
        "dependencies": ["provenance-core"],
        "host-dependencies": host_dependencies.clone(),
        "dev-dependencies": ["tempfile"]
    });
    policy
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn fixture_kernel_matches_its_policy() {
    assert_eq!(
        kernel(fixture_policy(), &fixture_metadata()),
        Vec::<String>::new()
    );
    assert_eq!(
        kernel(store_kernel_policy(&json!(["tokio"])), &fixture_metadata()),
        Vec::<String>::new()
    );
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn forbidden_row_without_a_host_crate_or_an_outside_crate_is_refused() {
    let mut policy = fixture_policy();
    policy["forbidden"]["provenance-core"] = json!([
        "fs2",
        "provenance-cli",
        "provenance-mcp",
        "provenance-store",
        "rustix",
        "tempfile",
        "windows-sys"
    ]);

    assert_eq!(
        kernel(policy, &fixture_metadata()),
        [
            "the forbidden row for kernel crate provenance-core in \
             .config/crate-dependencies.json does not name tokio",
            "the forbidden row for kernel crate provenance-core in \
             .config/crate-dependencies.json does not name provenance-report",
        ]
    );
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn default_feature_that_reaches_host_is_refused() {
    let mut metadata = fixture_metadata();
    set_features(
        &mut metadata,
        "provenance-core",
        json!({ "default": ["schema"], "schema": ["host"], "host": [] }),
    );

    assert_eq!(
        kernel(fixture_policy(), &metadata),
        ["the default feature of provenance-core turns on host"]
    );
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn edge_that_turns_on_host_in_a_kernel_crate_is_refused() {
    let mut metadata = fixture_metadata();
    let packages = metadata["packages"].as_array_mut().unwrap();
    let store = packages
        .iter_mut()
        .find(|package| package["name"] == "provenance-store");
    store.unwrap()["dependencies"][0]["features"] = json!(["host"]);

    assert_eq!(
        kernel(
            store_kernel_policy(&json!(["fs2", "rustix", "tokio"])),
            &metadata
        ),
        [
            "the default feature of provenance-store turns on host in provenance-core",
            "provenance-store has a build dependency on fs2 in host-dependencies that is not \
             optional",
            "provenance-store has a normal dependency on rustix in host-dependencies that is not \
             optional",
        ]
    );
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn host_dependency_outside_the_host_feature_is_refused() {
    let mut metadata = fixture_metadata();
    set_features(
        &mut metadata,
        "provenance-store",
        json!({ "default": ["tokio"], "host": [], "tokio": ["dep:tokio"] }),
    );
    let policy = store_kernel_policy(&json!(["tokio"]));
    assert_eq!(
        kernel(policy, &metadata),
        ["the default feature of provenance-store turns on its host dependency tokio"]
    );

    set_features(
        &mut metadata,
        "provenance-store",
        json!({ "host": [], "tokio": ["dep:tokio"] }),
    );
    let policy = store_kernel_policy(&json!(["tokio"]));
    assert_eq!(
        kernel(policy, &metadata),
        ["the host feature of provenance-store does not turn on its host dependency tokio"]
    );
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn host_dependencies_of_a_crate_outside_the_kernel_are_refused() {
    let mut policy = fixture_policy();
    policy["crates"]["provenance-report"]["host-dependencies"] = json!(["provenance-store"]);

    assert_eq!(
        kernel(policy, &fixture_metadata()),
        [
            ".config/crate-dependencies.json gives provenance-report host-dependencies, but \
          provenance-report is not a kernel crate"
        ]
    );
}

#[test]
#[verifies("rule_kernel_default_features_have_no_host_crate", examples)]
fn kernel_target_that_is_not_wasm32_is_refused() {
    let mut policy = fixture_policy();
    policy["kernel"]["target"] = json!("x86_64-unknown-linux-gnu");

    assert_eq!(
        kernel(policy, &fixture_metadata()),
        ["the kernel target x86_64-unknown-linux-gnu in .config/crate-dependencies.json is not a \
          wasm32 target"]
    );
}
