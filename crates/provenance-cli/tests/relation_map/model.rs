//! The row shape of the relation map and the runner that drives it.
//!
//! A row names the act that creates the relation, where the fact is read
//! back, and how it is cleared. `{name}` in any argument or pointer is
//! replaced by a value an earlier row captured from its own output.

use assert_cmd::Command;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct Row {
    pub number: u8,
    pub relation: &'static str,
    pub owner: &'static str,
    pub act: Act,
    pub read: Read,
    pub clear: Clear,
}

pub enum Act {
    /// A CLI invocation (`--repo` and `--scope default` are added).
    Cli(&'static [&'static str]),
    /// An `sdk` operation fed one JSON document on stdin.
    Sdk(&'static str, Value),
    /// The relation is written by another row's act, or by a create in setup.
    Derived(&'static str),
    /// No authoring path exists; the reason is recorded.
    Excluded(&'static str),
}

pub enum Read {
    /// A canonical graph record through `sdk get`.
    Record {
        kind: &'static str,
        id: &'static str,
        pointer: &'static str,
        expect: Value,
    },
    /// A JSON pointer into this row's own act output.
    Output {
        pointer: &'static str,
        expect: Value,
        capture: &'static [(&'static str, &'static str)],
    },
    /// A JSON pointer into an earlier row's act output.
    OutputOf {
        row: u8,
        pointer: &'static str,
        expect: Value,
    },
    /// An `sdk` read whose answer is checked at a pointer.
    Sdk {
        operation: &'static str,
        request: Value,
        pointer: &'static str,
        expect: Value,
    },
    None,
}

pub enum Clear {
    /// A CLI invocation, after which the pointer reads `expect`.
    Cli(&'static [&'static str], Value),
    /// An `sdk` operation, after which the sdk read answers `expect`.
    Sdk(&'static str, Value, Value),
    /// The record never changes this field after creation.
    Immutable,
    /// The field is set once and no command clears it.
    None,
}

pub struct Runner {
    pub repo: String,
    values: BTreeMap<String, String>,
    outputs: BTreeMap<u8, Value>,
}

impl Runner {
    pub const fn new(repo: String) -> Self {
        Self {
            repo,
            values: BTreeMap::new(),
            outputs: BTreeMap::new(),
        }
    }

    pub fn capture(&mut self, name: &str, value: &str) {
        self.values.insert(name.to_string(), value.to_string());
    }

    fn fill(&self, text: &str) -> String {
        let mut filled = text.to_string();
        for (name, value) in &self.values {
            filled = filled.replace(&format!("{{{name}}}"), value);
        }
        filled
    }

    fn fill_value(&self, value: &Value) -> Value {
        serde_json::from_str(&self.fill(&value.to_string())).unwrap()
    }

    pub fn cli(&self, args: &[&str]) -> Value {
        let mut command = Command::cargo_bin("provenance").unwrap();
        let filled: Vec<String> = args.iter().map(|arg| self.fill(arg)).collect();
        command
            .args(&filled)
            .args(["--repo", &self.repo, "--scope", "default"]);
        if !filled.iter().any(|arg| arg == "--format") {
            command.args(["--format", "json"]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{filled:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or(Value::Null)
    }

    pub fn sdk(&self, operation: &str, request: &Value) -> Value {
        let output = Command::cargo_bin("provenance")
            .unwrap()
            .args([
                "sdk", operation, "--repo", &self.repo, "--scope", "default", "--format", "json",
            ])
            .write_stdin(self.fill_value(request).to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "sdk {operation} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn record(&self, kind: &str, id: &str) -> Value {
        let id = self.fill(id);
        let answer = self.sdk(
            "get",
            &json!({"node_type": kind, "id": id, "include_retired": true}),
        );
        assert_eq!(answer["found"], true, "{kind} {id} exists");
        answer["node"].clone()
    }

    fn check(&self, row: &Row, value: &Value, pointer: &str, expect: &Value) {
        let found = value
            .pointer(&self.fill(pointer))
            .cloned()
            .unwrap_or(Value::Null);
        assert_eq!(
            found,
            self.fill_value(expect),
            "row {} ({} on {}) at {pointer}",
            row.number,
            row.relation,
            row.owner
        );
    }

    pub fn run(&mut self, row: &Row) {
        let output = match &row.act {
            Act::Cli(args) => self.cli(args),
            Act::Sdk(operation, request) => self.sdk(operation, request),
            Act::Derived(reason) | Act::Excluded(reason) => {
                assert!(!reason.is_empty(), "row {} states its reason", row.number);
                Value::Null
            }
        };
        match &row.read {
            Read::Record {
                kind,
                id,
                pointer,
                expect,
            } => {
                let record = self.record(kind, id);
                self.check(row, &record, pointer, expect);
            }
            Read::Output {
                pointer,
                expect,
                capture,
            } => {
                for (name, at) in *capture {
                    let value = output
                        .pointer(at)
                        .and_then(Value::as_str)
                        .unwrap()
                        .to_string();
                    self.capture(name, &value);
                }
                self.check(row, &output, pointer, expect);
            }
            Read::OutputOf {
                row: earlier,
                pointer,
                expect,
            } => {
                let earlier_output = self.outputs[earlier].clone();
                self.check(row, &earlier_output, pointer, expect);
            }
            Read::Sdk {
                operation,
                request,
                pointer,
                expect,
            } => {
                let answer = self.sdk(operation, request);
                self.check(row, &answer, pointer, expect);
            }
            Read::None => assert!(
                matches!(row.act, Act::Excluded(_)),
                "row {} reads nothing back",
                row.number
            ),
        }
        self.outputs.insert(row.number, output);
        match &row.clear {
            Clear::Cli(args, expect) => {
                self.cli(args);
                let Read::Record {
                    kind, id, pointer, ..
                } = &row.read
                else {
                    panic!("row {} clears a field it does not read back", row.number);
                };
                let record = self.record(kind, id);
                self.check(row, &record, pointer, expect);
            }
            Clear::Sdk(operation, request, expect) => {
                let cleared = self.sdk(operation, request);
                match &row.read {
                    Read::Sdk {
                        operation,
                        request,
                        pointer,
                        ..
                    } => {
                        let answer = self.sdk(operation, request);
                        self.check(row, &answer, pointer, expect);
                    }
                    Read::Record {
                        kind, id, pointer, ..
                    } => {
                        let record = self.record(kind, id);
                        self.check(row, &record, pointer, expect);
                    }
                    Read::Output { pointer, .. } => self.check(row, &cleared, pointer, expect),
                    _ => panic!("row {} clears a field it does not read back", row.number),
                }
            }
            Clear::Immutable | Clear::None => {}
        }
    }
}
