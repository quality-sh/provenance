use assert_cmd::Command;
use provenance_macros::verifies;
use provenance_porcelain::guidance::guidance_for;
use serde_json::Value;

fn prime(repo: &std::path::Path, extra: &[&str]) -> String {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["prime", "--repo", repo.to_str().unwrap()])
        .args(extra)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

#[test]
#[verifies("rule_porcelain_cli_prime_teaches_domain", examples)]
fn prime_teaches_domain_without_reading_or_writing_repository_state() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".provenance/state")).unwrap();
    let manifest = repo.path().join(".provenance/state/manifest.json");
    std::fs::write(&manifest, "not a valid manifest").unwrap();

    let text = prime(repo.path(), &[]);
    for term in [
        "Requirement",
        "Rule",
        "Resolution",
        "Implementation binding",
        "Verification",
        "An active Rule can precede its implementation.",
        "A source citation does not count as an Implementation binding.",
        "A graph read does not scan code or establish a coverage verdict.",
    ] {
        assert!(text.contains(term), "guidance omits {term}: {text}");
    }
    let json: Value = serde_json::from_str(&prime(repo.path(), &["--format", "json"])).unwrap();
    assert_eq!(json["guidance"], text.trim_end());
    for field in ["rules", "requirements", "threads", "skills"] {
        assert!(
            json.get(field).is_none(),
            "prime contains project state: {field}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(manifest).unwrap(),
        "not a valid manifest"
    );
    assert_eq!(std::fs::read_dir(repo.path()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_dir(repo.path().join(".provenance"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_dir(repo.path().join(".provenance/state"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn legacy_state_options_leave_guidance_unchanged() {
    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    assert_eq!(
        text,
        prime(repo.path(), &["--scope", "old_scope", "--include-threads"])
    );
    assert_eq!(std::fs::read_dir(repo.path()).unwrap().count(), 0);
}

/// The record types, and Domain, which is no longer a record type. A guidance heading
/// does not use these words, so that a reader does not take a heading for a record type.
const NOT_HEADINGS: [&str; 10] = [
    "Domain",
    "Requirement",
    "Rule",
    "Resolution",
    "Source",
    "Topic",
    "Question",
    "Discussion",
    "Thread",
    "Message",
];

// An implementation aid with no Rule binding: rule_porcelain_cli_prime_teaches_domain and
// rule_porcelain_mcp_guidance_native each name one interface, and no Rule states the
// guidance headings.
#[tokio::test]
async fn cli_and_mcp_share_the_same_guidance() {
    use provenance_mcp::Client;

    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--path", repo.path().to_str().unwrap()])
        .assert()
        .success();
    let context = provenance_transport::repository::RepoContext::new(
        repo.path().to_str().unwrap(),
        "default",
    );
    let host = provenance_cli::porcelain::local_host(&context).unwrap();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = Client::connect(client_io).await.unwrap();
    let cli = guidance_for(&provenance_cli::exposed::shown(), &[], None).guidance;
    let (shared, _) = cli.split_once("\n## Actions\n").unwrap();
    let instructions = client.instructions().unwrap();
    assert!(text.starts_with(&cli), "{text}");
    assert!(instructions.starts_with(shared), "{instructions}");
    for guidance in [&text, &instructions] {
        let headings = guidance
            .lines()
            .filter_map(|line| line.strip_prefix("## "))
            .collect::<Vec<_>>();
        assert!(headings.contains(&"Records"), "{headings:?}");
        for heading in headings {
            assert!(
                !heading
                    .split_whitespace()
                    .any(|word| NOT_HEADINGS.contains(&word.trim_end_matches('s'))),
                "the heading {heading} reads as a record type"
            );
        }
    }
    for guidance in [&text, &instructions] {
        assert!(guidance.contains("\n\nFor a focused start, "), "{guidance}");
    }
    let tools = client.tools().await;
    for name in ["get", "search", "trace", "discussions"] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        let description = tool["description"].as_str().unwrap();
        assert!(
            text.contains(description),
            "shared description missing: {name}"
        );
    }
    assert!(tools.iter().all(|tool| tool["name"] != "prime"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

/// The output of `provenance <arguments>` in `repo`, when it exits with success.
fn run(repo: &std::path::Path, arguments: &[&str]) -> Option<String> {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(repo)
        .args(arguments)
        .output()
        .unwrap();
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).unwrap())
}

/// The help text of `provenance <words> --help`.
fn help(repo: &std::path::Path, words: &[&str]) -> String {
    let arguments = [words, &["--help"]].concat();
    run(repo, &arguments).unwrap_or_else(|| panic!("the CLI refuses {arguments:?}"))
}

/// The first word of each line of the `Commands:` section of a help text, without the
/// built-in `help`.
fn listed_commands(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|word| *word != "help")
        .map(str::to_owned)
        .collect()
}

/// Each command form that the help of the CLI shows: each command of the root help and of
/// its group helps, each value of the action after a record ID, and each `provenance` form
/// of the root help after its command list.
fn help_forms(repo: &std::path::Path) -> Vec<String> {
    let root = help(repo, &[]);
    let mut forms = Vec::new();
    let mut groups = vec![Vec::<String>::new()];
    while let Some(group) = groups.pop() {
        let words: Vec<&str> = group.iter().map(String::as_str).collect();
        let commands = listed_commands(&help(repo, &words));
        if commands.is_empty() {
            forms.push(group.join(" "));
        }
        groups.extend(
            commands
                .into_iter()
                .map(|command| [group.clone(), vec![command]].concat()),
        );
    }
    let record = help(repo, &["req_prime_form"]);
    let (_, actions) = record.split_once("[possible values: ").unwrap();
    let (actions, _) = actions.split_once(']').unwrap();
    forms.extend(actions.split(", ").map(|action| format!("<id> {action}")));
    let (_, footer) = root.split_once("\nOptions:\n").unwrap();
    for form in footer.split("provenance ").skip(1) {
        let mut words = form.split_whitespace();
        let first = words.next().unwrap().trim_end_matches([',', '.']);
        match first.strip_prefix('<') {
            Some(_) => {
                let actions = words.next().unwrap().trim_end_matches([',', '.']);
                forms.extend(actions.split('|').map(|action| format!("<id> {action}")));
            }
            None => forms.push(first.to_owned()),
        }
    }
    forms.sort();
    forms.dedup();
    forms
}

/// The names of the command lines of the guidance.
fn line_names(text: &str) -> Vec<&str> {
    let (_, actions) = text.split_once("\n## Actions\n").unwrap();
    actions
        .lines()
        .filter_map(|line| line.strip_prefix("- ")?.split_once(": "))
        .map(|(name, _)| name)
        .collect()
}

/// The command forms of the command lines of the guidance. A line that gives more than one
/// form joins them with ` or `.
fn line_forms(text: &str) -> Vec<&str> {
    line_names(text)
        .into_iter()
        .flat_map(|name| name.split(" or "))
        .collect()
}

/// The command of each `provenance` code span, without the options and their values.
fn code_forms(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|code| code.strip_prefix("provenance "))
        .map(|code| {
            code.split_whitespace()
                .take_while(|word| !word.starts_with('-'))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Whether the CLI accepts `provenance <form>`. A form that starts with `<id>` is accepted
/// when its action is accepted after a record ID. Another form is accepted when its words
/// select a command: its help is not the help of a record ID.
fn accepts(repo: &std::path::Path, form: &str) -> bool {
    let words: Vec<&str> = form.split_whitespace().collect();
    match words.as_slice() {
        ["<id>", action] => run(repo, &["req_prime_form", action, "--help"]).is_some(),
        root => {
            let record = help(repo, &["req_prime_form"]);
            let arguments = [root, &["--help"]].concat();
            run(repo, &arguments).is_some_and(|help| help != record)
        }
    }
}

/// Prime has a line for each command that the CLI help shows, in each form that selects it
/// on the CLI, and the CLI accepts each form that prime names.
#[test]
#[verifies("rule_guidance_names_accepted_commands", examples)]
fn prime_names_each_command_that_the_cli_shows_in_its_cli_form() {
    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    let named = line_forms(&text);
    let unnamed: Vec<String> = help_forms(repo.path())
        .into_iter()
        .filter(|form| !named.contains(&form.as_str()))
        .collect();
    assert!(
        unnamed.is_empty(),
        "prime does not name {unnamed:?}, which the CLI help shows:\n{text}"
    );
    let forms = code_forms(&text);
    assert!(
        forms.iter().any(|form| form.starts_with("<id> ")),
        "prime gives no CLI form of an action on a record:\n{text}"
    );
    for form in named
        .iter()
        .copied()
        .chain(forms.iter().map(String::as_str))
    {
        assert!(
            accepts(repo.path(), form),
            "the CLI refuses `provenance {form}`, which prime names:\n{text}"
        );
        let Some(action) = form.strip_prefix("<id> ") else {
            continue;
        };
        assert!(
            named.contains(&action) || !accepts(repo.path(), action),
            "the CLI accepts `provenance {action}` with no record ID:\n{text}"
        );
    }
}
