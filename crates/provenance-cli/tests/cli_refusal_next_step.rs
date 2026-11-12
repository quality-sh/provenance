//! Each refusal names the command that it refuses and the form that corrects the request.
//!
//! Implementation aid: no active Rule states these refusal texts.
//! `rule_cli_closed_value_refusal_lists_values` is in review, and
//! `rule_cli_one_discussion_read_form` states the read form, not the text of a refusal.

#[path = "support/bounded_process.rs"]
mod bounded_process;
use bounded_process::Bounded;

/// Clap writes the name of the executable file in the usage line. On Windows, this name
/// has the `.exe` suffix.
const SUFFIX: &str = std::env::consts::EXE_SUFFIX;

/// Runs the binary in an empty directory and gives the exit code, the standard output
/// and the standard error.
fn refusal(words: &[&str]) -> (Option<i32>, String, String) {
    let directory = tempfile::tempdir().unwrap();
    let output = bounded_process::provenance()
        .current_dir(directory.path())
        .args(words)
        .bounded_output();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn assert_refusal(words: &[&str], error: &str) {
    assert_eq!(
        refusal(words),
        (Some(2), String::new(), error.to_owned()),
        "{words:?}"
    );
}

#[test]
fn root_word_refusal_shows_the_usage_and_help_of_its_command() {
    assert_refusal(
        &["api", "requirements", "extra"],
        &format!(
            "error: unexpected argument 'extra' found\n\n\
             Usage: provenance{SUFFIX} api [OPTIONS] [PATH]\n\n\
             For more information, try 'provenance api --help'.\n"
        ),
    );
    assert_refusal(
        &["api", "--bogus"],
        &format!(
            "error: unexpected argument '--bogus' found\n\n  \
             tip: to pass '--bogus' as a value, use '-- --bogus'\n\n\
             Usage: provenance{SUFFIX} api [OPTIONS] [PATH]\n\n\
             For more information, try 'provenance api --help'.\n"
        ),
    );
    assert_refusal(
        &["api", "--method"],
        "error: a value is required for '--method <METHOD>' but none was supplied\n  \
         [possible values: get, post, patch]\n\n\
         For more information, try 'provenance api --help'.\n",
    );
    assert_refusal(
        &["search", "--text", "a", "--text", "b"],
        &format!(
            "error: the argument '--text <TEXT>' cannot be used multiple times\n\n\
             Usage: provenance{SUFFIX} search [OPTIONS]\n\n\
             For more information, try 'provenance search --help'.\n"
        ),
    );
}

#[test]
fn discussions_status_refusal_ends_with_a_newline() {
    assert_refusal(
        &["discussions", "--status", "bogus"],
        "error: invalid value for --status; accepted values: active, resolved, all\n",
    );
}

#[test]
fn positional_value_refusal_names_the_flag() {
    let cases: [(&[&str], &str, &str, &str); 9] = [
        (
            &["check", ""],
            "",
            "the statement with --statement",
            "check [OPTIONS]",
        ),
        (
            &["import", "x.json"],
            "x.json",
            "the scope export file with --input",
            "import [OPTIONS] --input <INPUT>",
        ),
        (
            &["export", "out.json"],
            "out.json",
            "the scope export file with --output",
            "export [OPTIONS]",
        ),
        (
            &["init", "sub"],
            "sub",
            "the repository with --path",
            "init [OPTIONS]",
        ),
        (
            &["validate", "proposal", "x.json"],
            "x.json",
            "the artifact file with --input",
            "validate [OPTIONS] --input <INPUT> <ARTIFACT>",
        ),
        (
            &["dictionary", "import", "x.pdf"],
            "x.pdf",
            "the PDF file with --pdf",
            "dictionary import [OPTIONS] --pdf <PDF>",
        ),
        (
            &["prime", "markdown"],
            "markdown",
            "the format with --format",
            "prime [OPTIONS]",
        ),
        (
            &["mcp", "repo"],
            "repo",
            "the repository with --repo",
            "mcp [OPTIONS]",
        ),
        (
            &["review", "repo"],
            "repo",
            "the repository with --repo",
            "review [OPTIONS] --repo <REPO> --repository-id <REPOSITORY_ID> --scope <SCOPE>",
        ),
    ];
    for (words, found, tip, usage) in cases {
        assert_refusal(
            words,
            &format!(
                "error: unexpected argument '{found}' found\n\n  \
                 tip: give {tip}\n\n\
                 Usage: provenance{SUFFIX} {usage}\n\n\
                 For more information, try '--help'.\n"
            ),
        );
    }
}

#[test]
fn refusal_without_a_table_flag_stays_as_clap_gives_it() {
    assert_refusal(
        &["import", "-x"],
        &format!(
            "error: unexpected argument '-x' found\n\n\
             Usage: provenance{SUFFIX} import [OPTIONS] --input <INPUT>\n\n\
             For more information, try '--help'.\n"
        ),
    );
    assert_refusal(
        &["materialize", "x"],
        &format!(
            "error: unexpected argument 'x' found\n\n\
             Usage: provenance{SUFFIX} materialize [OPTIONS]\n\n\
             For more information, try '--help'.\n"
        ),
    );
}

#[test]
fn positional_value_refusal_gives_no_tip_when_the_flag_is_present() {
    let cases: [(&[&str], &str, &str); 5] = [
        (
            &["import", "x.json", "--input", "y.json"],
            "x.json",
            "import [OPTIONS] --input <INPUT>",
        ),
        (
            &["import", "--input", "y.json", "x.json"],
            "x.json",
            "import [OPTIONS] --input <INPUT>",
        ),
        (&["init", "--path", "a", "b"], "b", "init [OPTIONS]"),
        (&["prime", "--format", "json", "x"], "x", "prime [OPTIONS]"),
        (&["prime", "--format=json", "x"], "x", "prime [OPTIONS]"),
    ];
    for (words, found, usage) in cases {
        assert_refusal(
            words,
            &format!(
                "error: unexpected argument '{found}' found\n\n\
                 Usage: provenance{SUFFIX} {usage}\n\n\
                 For more information, try '--help'.\n"
            ),
        );
    }
}
