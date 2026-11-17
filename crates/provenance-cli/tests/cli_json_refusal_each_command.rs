//! Each command that the CLI defines and that takes `--format json` prints a
//! refusal as one `{error,meta}` object on standard error. The test reads the
//! commands from the declarations that the CLI renders its parser from, so a new
//! command is in the walk, and the table of outcomes must name its line.

use provenance_cli::cli_options::{self, flag_name};
use provenance_macros::verifies;
use provenance_porcelain::registry::{Declaration, Field, FieldValue};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

use crate::bounded_process;
use crate::support::review_paths::init_scope;

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

/// The exit code and standard error of one command line, run in an initialized
/// repository with no home files.
fn run(line: &[String]) -> (Option<i32>, String) {
    let directory = tempfile::tempdir().unwrap();
    let work = directory.path().join("work");
    let home = directory.path().join("home");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    init_scope(&work, "default", ".");
    std::fs::write(work.join(DAMAGED), "not json\n").unwrap();
    let mut command = bounded_process::provenance();
    command
        .current_dir(&work)
        .env_remove("PROVENANCE_REPO")
        .env_remove("PROVENANCE_SCOPE")
        .env_remove("CODEX_THREAD_ID")
        .envs(home_variables(&home))
        .args(line);
    let output = bounded_process::output(&mut command, Duration::from_secs(120));
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

/// The kind of the refusal when standard error is one `{error,meta}` object
/// with no prefix.
fn refusal_kind(stderr: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(stderr).ok()?;
    let kind = value["error"]["kind"].as_str()?;
    (stderr.trim_end().lines().count() == 1 && value["meta"].is_object()).then(|| kind.to_owned())
}

/// The outcome of each line of the walk: the kind of its refusal object, or
/// [`ACCEPTED`] for a line that the command accepts. The `dogfood` lines are in
/// the walk only with the `dogfood` feature.
const OUTCOMES: &[(&str, &str)] = &[
    ("record_x get --kind source --format json", "invalid_input"),
    ("search --kind source --format json", ACCEPTED),
    ("api damaged --format json", "unknown_operation"),
    ("check --format json", ACCEPTED),
    (
        "record_x trace --kind source --format json",
        "resource_not_found",
    ),
    ("prime --format json", ACCEPTED),
    (
        "record_x create --type source --format json",
        "invalid_input",
    ),
    ("record_x update --format json", "resource_not_found"),
    ("record_x answer --format json", "resource_not_found"),
    ("record_x claim --format json", "resource_not_found"),
    ("record_x release --format json", "resource_not_found"),
    ("record_x submit --format json", "resource_not_found"),
    ("discussions --format json", ACCEPTED),
    ("record_x discussions --format json", "resource_not_found"),
    ("record_x discussion --format json", "resource_not_found"),
    ("record_x discuss --format json", "resource_not_found"),
    ("record_x reply --format json", "invalid_input"),
    ("materialize --format json", ACCEPTED),
    ("export --format json", ACCEPTED),
    ("import --input damaged --format json", "operation_failed"),
    (
        "merge-jsonl damaged damaged damaged --format json",
        "invalid_state_file",
    ),
    (
        "validate contribution --input damaged --format json",
        "operation_failed",
    ),
    ("schema show contribution --format json", ACCEPTED),
    (
        "dictionary import --pdf damaged --format json",
        "operation_failed",
    ),
    ("dictionary status --format json", ACCEPTED),
    (
        "swarm-backtrace land --run-dir damaged --format json",
        "operation_failed",
    ),
    ("skills list --format json", ACCEPTED),
    ("skills install --format json", ACCEPTED),
    (
        "report render --input damaged --format json",
        "operation_failed",
    ),
    (
        "report build --base damaged --head damaged --repository damaged --format json",
        "operation_failed",
    ),
    ("dogfood list --format json", ACCEPTED),
    ("dogfood report --format json", ACCEPTED),
    ("dogfood triage list --format json", ACCEPTED),
];

/// The outcome of a line that the command accepts with exit code 0.
const ACCEPTED: &str = "accepted";

/// Each declared command that accepts `--format json` and refuses the walk's line
/// prints the refusal object with the kind that the table names. A line that
/// prints text, a new line, or a line with a different outcome fails the test.
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
    let mut outcomes = Vec::new();
    for declaration in declarations {
        for (form, on_record) in forms(&declaration) {
            let line = json_line(&declaration, &form, on_record);
            let (code, stderr) = run(&line);
            let shown = line.join(" ");
            let outcome = match code {
                Some(2) if stderr.contains("unexpected argument '--format'") => continue,
                Some(0) => ACCEPTED.to_owned(),
                Some(2) => panic!("{shown}: the parser refused the line: {stderr}"),
                _ => refusal_kind(&stderr).unwrap_or_else(|| {
                    panic!("{shown}: a text refusal with --format json: exit {code:?}: {stderr}")
                }),
            };
            outcomes.push((shown, outcome));
        }
    }
    let expected = outcomes
        .iter()
        .map(|(shown, _)| {
            let outcome = OUTCOMES.iter().find(|(line, _)| line == shown);
            (
                shown.clone(),
                outcome.map_or_else(String::new, |(_, kind)| (*kind).to_owned()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes,
        expected,
        "the outcome of each line:\n{}",
        outcomes
            .iter()
            .map(|(shown, outcome)| format!("    ({shown:?}, {outcome:?}),"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
