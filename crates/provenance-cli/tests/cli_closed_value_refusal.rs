//! A CLI flag or positional argument whose value comes from a closed set refuses a
//! value outside that set, and the refusal shows each value of the set.

use assert_cmd::Command;
use provenance_cli::cli_options::{self, flag_name, GLOBAL_OPTIONS};
use provenance_macros::verifies;
use provenance_porcelain::registry::{Declaration, Field};

const KINDS: &str = "source, requirement, resolution, rule, topic, question";

/// The exit code and the standard error of one refused command line.
fn refusal(args: &[&str]) -> (Option<i32>, String) {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .current_dir(directory.path())
        .args(args)
        .output()
        .unwrap();
    assert!(output.stdout.is_empty(), "{args:?} wrote to standard output");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The full refusal that clap gives for a value outside the possible values.
fn clap_refusal(arg: &str, values: &str) -> String {
    format!(
        "error: invalid value 'bogus' for '{arg}'\n  [possible values: {values}]\n\n\
         For more information, try '--help'.\n"
    )
}

#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn search_kind_refusal_lists_the_record_kinds() {
    assert_eq!(
        refusal(&["search", "--text", "a", "--kind", "bogus"]),
        (Some(2), clap_refusal("--kind <KIND>", KINDS))
    );
}

#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn get_kind_refusal_lists_the_record_kinds() {
    assert_eq!(
        refusal(&["req_x", "get", "--view", "children", "--kind", "bogus"]),
        (Some(2), clap_refusal("--kind <KIND>", KINDS))
    );
}

#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn create_type_refusal_lists_the_record_kinds() {
    assert_eq!(
        refusal(&["rule_y", "create", "--type", "bogus"]),
        (Some(2), clap_refusal("--type <TYPE>", KINDS))
    );
}

#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn api_method_refusal_lists_the_methods() {
    assert_eq!(
        refusal(&["api", "requirements", "--method", "bogus"]),
        (Some(2), clap_refusal("--method <METHOD>", "get, post, patch"))
    );
}

#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn skills_show_refusal_lists_the_bundled_skills() {
    assert_eq!(
        refusal(&["skills", "show", "bogus"]),
        (
            Some(1),
            "Error: unknown skill: bogus; accepted values: provenance-fork-tournament, \
             provenance-grounded-writing, provenance-shaping, provenance-swarm-backtrace\n"
                .to_owned()
        )
    );
}

/// The command line of one declared command up to its fields: its words, or a record
/// id and the action name for a command that takes a target.
fn command_line(declaration: &Declaration) -> Vec<String> {
    cli_options::words(declaration).map_or_else(
        || vec!["record_x".to_owned(), declaration.name.to_owned()],
        |words| words.into_iter().map(str::to_owned).collect(),
    )
}

/// The command line that gives `bogus` to one field of a declared command.
fn bogus_line(declaration: &Declaration, field: &Field) -> Vec<String> {
    let mut line = command_line(declaration);
    let target = declaration.input.target();
    match target.iter().position(|name| *name == field.name) {
        Some(index) => {
            line.extend(target[..index].iter().map(|_| "x".to_owned()));
            line.push("bogus".to_owned());
        }
        None => line.extend([format!("--{}", flag_name(field)), "bogus".to_owned()]),
    }
    line
}

/// Each declared field of each command that the CLI shows, and each shared option, with
/// its closed set from the registry: every field that the CLI gives a closed-set parser.
#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn each_declared_closed_set_field_lists_its_values() {
    let get = provenance_porcelain::registry::Command::Get.declaration();
    let declarations = provenance_cli::exposed::EXPOSED
        .iter()
        .map(|command| command.declaration())
        .chain(
            provenance_cli::servers::SERVERS
                .iter()
                .map(|server| server.declaration()),
        );
    let mut cases = Vec::new();
    for declaration in declarations {
        let options = cli_options::command_options(&declaration).0;
        for field in declaration.input.fields().iter().chain(options) {
            cases.push((bogus_line(&declaration, field), *field));
        }
    }
    cases.extend(
        GLOBAL_OPTIONS
            .iter()
            .map(|field| (bogus_line(&get, field), *field)),
    );
    let mut checked = Vec::new();
    for (line, field) in cases {
        let values = field.closed_values();
        if values.is_empty() {
            continue;
        }
        let args = line.iter().map(String::as_str).collect::<Vec<_>>();
        let (code, stderr) = refusal(&args);
        assert_eq!(code, Some(2), "{args:?}: {stderr}");
        assert!(
            stderr.starts_with("error: invalid value 'bogus' for '")
                && stderr.contains(&format!("\n  [possible values: {}]\n", values.join(", "))),
            "{args:?} does not list {values:?}: {stderr}"
        );
        checked.push(field.name);
    }
    for name in ["kind", "type", "method", "view", "format", "artifact"] {
        assert!(checked.contains(&name), "no closed-set case for {name}");
    }
}
