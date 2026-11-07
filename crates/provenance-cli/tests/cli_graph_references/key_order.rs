//! The four graph-reference commands print their JSON keys in the declared field order.
//! The order is part of the bytes of an exact export, so these pins hold it.

use super::*;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A JSON document that keeps the order of the object keys as the text gives them.
enum Node {
    Object(Vec<(String, Self)>),
    Array(Vec<Self>),
    Leaf,
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Node, E> {
        Ok(Node::Leaf)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Node, E> {
        Ok(Node::Leaf)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Node, E> {
        Ok(Node::Leaf)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Node, E> {
        Ok(Node::Leaf)
    }

    fn visit_str<E>(self, _: &str) -> Result<Node, E> {
        Ok(Node::Leaf)
    }

    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Leaf)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(Node::Object(entries))
    }
}

impl Node {
    fn parse(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap()
    }

    /// The value at `path`, one object key for each step.
    fn at(&self, path: &[&str]) -> &Self {
        path.iter().fold(self, |node, key| match node {
            Self::Object(entries) => {
                &entries
                    .iter()
                    .find(|(name, _)| name == key)
                    .unwrap_or_else(|| panic!("no key {key}"))
                    .1
            }
            _ => panic!("{key} is not in an object"),
        })
    }

    /// The keys of the object at `path`, in text order.
    fn keys(&self, path: &[&str]) -> Vec<&str> {
        match self.at(path) {
            Self::Object(entries) => entries.iter().map(|(name, _)| name.as_str()).collect(),
            _ => panic!("{path:?} is not an object"),
        }
    }

    /// The items of the array at `path`.
    fn items(&self, path: &[&str]) -> &[Self] {
        match self.at(path) {
            Self::Array(items) => items,
            _ => panic!("{path:?} is not an array"),
        }
    }
}

fn stdout(repo: &Path, args: &[&str]) -> Vec<u8> {
    provenance(repo)
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone()
}

const REFERENCE_KEYS: &[&str] = &[
    "reference_id",
    "repository_id",
    "store_path",
    "scope_id",
    "commit",
    "graph_digest",
    "correlation",
];
const FAMILY_KEYS: &[&str] = &[
    "sources",
    "requirements",
    "boundaries",
    "topics",
    "questions",
    "resolutions",
    "rules",
];

#[test]
fn graph_reference_commands_print_keys_in_declared_order() {
    let temp = committed_store();
    let repo = temp.path();
    let scoped = ["--repo", ".", "--scope", "default"];
    let source = [
        "source_one",
        "create",
        "--type",
        "source",
        "--name",
        "One source",
    ];
    stdout(repo, &[&source[..], &scoped].concat());
    let requirement = [
        "req_one",
        "create",
        "--type",
        "requirement",
        "--statement",
        "The host starts.",
    ];
    stdout(repo, &[&requirement[..], &scoped].concat());
    git(repo, &["add", ".provenance/state"]);
    git(repo, &["commit", "-qm", "one source and one requirement"]);

    let issued = stdout(
        repo,
        &[
            &["graph-reference", "issue"][..],
            &scoped,
            &["--correlation-system", "gh", "--correlation-key", "1"],
        ]
        .concat(),
    );
    let reference = Node::parse(&issued);
    assert_eq!(reference.keys(&[]), REFERENCE_KEYS);
    assert_eq!(reference.keys(&["correlation"]), ["system", "key"]);
    let path = repo.join("reference.json");
    std::fs::write(&path, &issued).unwrap();
    let path = path.to_str().unwrap();
    let read = |command: &str| {
        Node::parse(&stdout(
            repo,
            &[
                "graph-reference",
                command,
                "--repo",
                ".",
                "--reference",
                path,
            ],
        ))
    };

    let show = read("show");
    assert_eq!(show.keys(&[]), ["operation", "reference", "counts"]);
    assert_eq!(show.keys(&["reference"]), REFERENCE_KEYS);
    assert_eq!(show.keys(&["reference", "correlation"]), ["system", "key"]);
    assert_eq!(show.keys(&["counts"]), FAMILY_KEYS);

    let verify = read("verify");
    assert_eq!(verify.keys(&[]), ["operation", "valid", "reference_id"]);

    let export = read("exact-export");
    assert_eq!(
        export.keys(&[]),
        ["operation", "reference_id", "graph_digest", "graph"]
    );
    assert_eq!(
        export.keys(&["graph"]),
        [&["scope"][..], FAMILY_KEYS].concat()
    );
    assert_eq!(export.keys(&["graph", "scope"]), ["id", "path_prefix"]);
    let sources = export.items(&["graph", "sources"]);
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].keys(&[]),
        [
            "scope_id",
            "id",
            "created",
            "updated",
            "name",
            "source_type",
            "url"
        ]
    );
    let requirements = export.items(&["graph", "requirements"]);
    assert_eq!(requirements.len(), 1);
    assert_eq!(
        requirements[0].keys(&[]),
        [
            "scope_id",
            "id",
            "created",
            "updated",
            "statement",
            "status"
        ]
    );
}
