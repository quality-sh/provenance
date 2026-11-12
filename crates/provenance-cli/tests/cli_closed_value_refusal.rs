//! A CLI flag whose value comes from a closed set refuses a value outside that set,
//! and the refusal shows each value of the set. The `skills show` refusal also shows
//! its list, but no Rule covers a positional argument.

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
    assert!(
        output.stdout.is_empty(),
        "{args:?} wrote to standard output"
    );
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The full refusal that clap gives for a value outside the possible values.
fn clap_refusal(arg: &str, values: &str) -> String {
    help_refusal(arg, values, "--help")
}

/// The refusal for a value outside the possible values, with the help command that it
/// names.
fn help_refusal(arg: &str, values: &str, help: &str) -> String {
    format!(
        "error: invalid value 'bogus' for '{arg}'\n  [possible values: {values}]\n\n\
         For more information, try '{help}'.\n"
    )
}

#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn search_kind_refusal_lists_the_record_kinds() {
    assert_eq!(
        refusal(&["search", "--text", "a", "--kind", "bogus"]),
        (
            Some(2),
            help_refusal("--kind <KIND>", KINDS, "provenance search --help")
        )
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
        (
            Some(2),
            help_refusal(
                "--method <METHOD>",
                "get, post, patch",
                "provenance api --help"
            )
        )
    );
}

/// Implementation aid: no Rule covers a positional argument; the test pins the list in
/// the `skills show` refusal.
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

/// Implementation aid: the Rule is about refusals. The test shows that each closed-set
/// flag still accepts its value in a different letter case: the command gets past the
/// parse and stops because the directory has no graph.
#[test]
fn closed_set_flags_accept_a_value_in_any_letter_case() {
    for args in [
        &["search", "--text", "a", "--kind", "RULE"][..],
        &["search", "--text", "a", "--kind", " rule "],
        &["rule_y", "create", "--type", "Rule"],
        &["api", "requirements", "--method", "GET"],
    ] {
        assert_eq!(
            refusal(args),
            (
                Some(1),
                "Error: Provenance graph is not initialized; run `provenance init`\n".to_owned()
            ),
            "{args:?}"
        );
    }
}

/// Implementation aid: no Rule covers the help layout. In the help of each record type,
/// `--type` does not list the types, so each option keeps its help on its own row.
#[test]
fn per_type_create_help_keeps_one_row_for_each_option() {
    for kind in KINDS.split(", ") {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .args(["record_x", "create", "--type", kind, "--help"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{kind}");
        let help = String::from_utf8_lossy(&output.stdout);
        let repo = help
            .lines()
            .find(|line| line.trim_start().starts_with("--repo <REPO>"))
            .unwrap_or_else(|| panic!("{kind} help has no --repo row:\n{help}"));
        assert!(
            repo.ends_with("[default: .]"),
            "{kind} help moves the option help to the next line:\n{help}"
        );
    }
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

/// Each declared field of each command that the CLI shows, and each shared option, that
/// has a closed set in the registry gives a refusal that lists the set. The test reads the
/// expected list from `Field::closed_values`, which is also the source of the parser, so it
/// does not prove that the list is correct; the tests above pin the lists by hand. It does
/// not cover the catalog record flags, `skills show`, or the `[ACTION]` argument.
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
