//! The crate dependency policy file and its parts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub const POLICY_FILE: &str = ".config/crate-dependencies.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The crates that build for a wasm target with their default features.
    pub kernel: Kernel,
    /// Each interface crate, such as clap, with the one workspace crate that can depend on it.
    pub interface_crates: BTreeMap<String, String>,
    pub host_crates: BTreeSet<String>,
    /// Normal and build edges that no entry can allow. A row can name a crate
    /// that is not a member yet.
    pub forbidden: BTreeMap<String, BTreeSet<String>>,
    pub crates: BTreeMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kernel {
    pub target: String,
    /// The feature that turns on the host code of a kernel crate.
    pub host_feature: String,
    pub crates: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Entry {
    pub dependencies: BTreeSet<String>,
    /// Optional normal edges of a kernel crate that only its host feature turns on.
    #[serde(default)]
    pub host_dependencies: BTreeSet<String>,
    pub dev_dependencies: BTreeSet<String>,
}

impl Entry {
    /// The lists in the policy file, with the edge list that each one holds.
    pub const LISTS: [(&'static str, &'static str); 3] = [
        ("dependencies", "dependencies"),
        ("host-dependencies", "dependencies"),
        ("dev-dependencies", "dev-dependencies"),
    ];

    pub fn list(&self, list: &str) -> &BTreeSet<String> {
        match list {
            "dev-dependencies" => &self.dev_dependencies,
            "host-dependencies" => &self.host_dependencies,
            _ => &self.dependencies,
        }
    }

    /// Normal and build edges can be in `dependencies` or `host-dependencies`.
    pub fn allows(&self, edge_list: &str, dependency: &str) -> bool {
        if edge_list == "dev-dependencies" {
            self.dev_dependencies.contains(dependency)
        } else {
            self.dependencies.contains(dependency) || self.host_dependencies.contains(dependency)
        }
    }
}

pub fn read_policy() -> Policy {
    let policy = fs::read_to_string(workspace_root().join(POLICY_FILE)).expect("read policy");
    serde_json::from_str(&policy).expect("parse policy")
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate is in the workspace crates directory")
        .to_owned()
}
