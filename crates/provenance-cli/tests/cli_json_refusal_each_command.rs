//! Each command that the CLI defines and that takes `--format json` prints a
//! refusal as one `{error,meta}` object on standard error. The test reads the
//! commands from the declarations that the CLI renders its parser from, so a new
//! command is in the walk without a change here.

use provenance_cli::cli_options::{self, flag_name};
use provenance_macros::verifies;
use provenance_porcelain::registry::{Declaration, Field, FieldValue};
use serde_json::Value;
use std::path::Path;

use crate::support::review_paths::provenance;

/// The text of each path that a command line gives: a file that is not JSON,
/// not a PDF, and not a directory, so that a command that reads it refuses.
const DAMAGED: &str = "damaged";

/// The command line of each form that selects the command of `declaration`: its
/// words, and a record id with its name when it acts on a record.
fn forms(declaration: &Declaration) -> Vec<(Vec<String>, bool)> {
    let named = cli_options::words(declaration)
        .map(|words| (words.into_iter().map(str::to_owned).collect(), false));
    let on_record = declaration.grammar.takes_target().then(|| {
        (
            vec!["record_x".to_owned(), declaration.name.to_owned()],
            true,
        )
    });
    named.into_iter().chain(on_record).collect()
}

/// A value for one field that lets the command reach its operation.
fn value(field: &Field) -> String {
    match (field.closed_values().first(), field.value) {
        (Some(value), _) => (*value).to_owned(),
        (None, FieldValue::Count | FieldValue::CatalogLimit | FieldValue::Port) => "1".to_owned(),
        (None, _) => DAMAGED.to_owned(),
    }
}

/// The command line that gives each required field of `declaration` and each
/// field that names a record kind, then `--format json`.
fn json_line(declaration: &Declaration, form: &[String], on_record: bool) -> Vec<String> {
    let input = &declaration.input;
    let mut line = form.to_vec();
    for name in input.target() {
        if !on_record {
            let field = input.fields().iter().find(|field| field.name == *name);
            line.push(field.map_or_else(|| DAMAGED.to_owned(), value));
        }
    }
    for field in input.fields() {
        let given = field.required || field.value == FieldValue::RecordKind;
        if !given || input.target().contains(&field.name) {
            continue;
        }
        line.push(format!("--{}", flag_name(field)));
        if field.value != FieldValue::Switch {
            line.push(value(field));
        }
    }
    line.extend(["--format".to_owned(), "json".to_owned()]);
    line
}

/// The exit code and standard error of one command line, run in a new directory
/// that has no graph and no home files.
fn run(line: &[String]) -> (Option<i32>, String) {
    let directory = tempfile::tempdir().unwrap();
    let work = directory.path().join("work");
    let home = directory.path().join("home");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(work.join(DAMAGED), "not json\n").unwrap();
    let output = provenance()
        .current_dir(&work)
        .env_remove("PROVENANCE_REPO")
        .env_remove("PROVENANCE_SCOPE")
        .envs(home_variables(&home))
        .args(line)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The variables that point the home, data and spool directories into `home`.
fn home_variables(home: &Path) -> Vec<(&'static str, std::path::PathBuf)> {
    [
        "HOME",
        "USERPROFILE",
        "LOCALAPPDATA",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "CODEX_HOME",
        "PROVENANCE_DOGFOOD_DIR",
    ]
    .into_iter()
    .map(|name| (name, home.join(name.to_lowercase())))
    .collect()
}

/// Whether standard error is one `{error,meta}` object with a kind and no prefix.
fn is_refusal_object(stderr: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(stderr) else {
        return false;
    };
    stderr.trim_end().lines().count() == 1
        && value["error"]["kind"].is_string()
        && value["meta"].is_object()
}

/// Each declared command that accepts `--format json` and refuses the walk's line
/// prints the refusal object. A command that the line does not refuse has nothing to
/// check; the walk asserts that most commands refuse, so that it checks the rule.
#[verifies("rule_cli_json_refusal_object", examples)]
#[test]
fn each_command_with_json_output_prints_a_refusal_as_one_object() {
    let declarations = provenance_cli::exposed::EXPOSED
        .iter()
        .map(|command| command.declaration())
        .chain(
            provenance_cli::servers::SERVERS
                .iter()
                .map(|server| server.declaration()),
        );
    let (mut refused, mut text, mut accepted) = (Vec::new(), Vec::new(), Vec::new());
    for declaration in declarations {
        for (form, on_record) in forms(&declaration) {
            let line = json_line(&declaration, &form, on_record);
            let (code, stderr) = run(&line);
            let shown = line.join(" ");
            match code {
                Some(2) if stderr.contains("unexpected argument '--format'") => {}
                Some(0) => accepted.push(shown),
                Some(2) => panic!("{shown}: the parser refused the line: {stderr}"),
                _ if is_refusal_object(&stderr) => refused.push(shown),
                _ => text.push(format!("{shown}: exit {code:?}: {stderr}")),
            }
        }
    }
    assert!(
        text.is_empty(),
        "these commands print a text refusal with --format json:\n{}",
        text.join("\n")
    );
    assert!(
        refused.len() > accepted.len(),
        "the walk refused only {refused:?}; it accepted {accepted:?}"
    );
}
