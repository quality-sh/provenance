use assert_cmd::Command;
use provenance_macros::verifies;
use provenance_porcelain::guidance::guidance_for;
use provenance_porcelain::registry::Command as Porcelain;
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
    let cli = guidance_for(&provenance_cli::exposed::forms(), &[], false).guidance;
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
    assert!(text.contains("`provenance search --text <text>"), "{text}");
    assert!(
        instructions.contains("call search with {\"text\":\"<text>\"}"),
        "{instructions}"
    );
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

/// The form that selects `command` on the CLI: the words of its name, or `<id>` and its
/// name for a command that only acts on a record.
fn cli_form(command: Porcelain) -> String {
    let declaration = command.declaration();
    if declaration.grammar.takes_name() {
        declaration.name.to_owned()
    } else {
        format!("<id> {}", declaration.name)
    }
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

/// Whether the CLI accepts `provenance <form>`: the root words select a command that the
/// CLI shows, or the form is a record ID and an action that the CLI accepts after it.
fn accepts(form: &str) -> bool {
    let words: Vec<&str> = form.split_whitespace().collect();
    match words.as_slice() {
        ["<id>", action] => Command::cargo_bin("provenance")
            .unwrap()
            .args(["req_prime_form", action, "--help"])
            .output()
            .unwrap()
            .status
            .success(),
        root => provenance_cli::exposed::find(root).is_some(),
    }
}

/// Prime has a line for each command that the CLI help lists, in the form that selects it
/// on the CLI, and the CLI accepts each command that prime names.
#[test]
#[verifies("rule_guidance_names_accepted_commands", examples)]
fn prime_names_each_command_that_the_cli_shows_in_its_cli_form() {
    let repo = tempfile::tempdir().unwrap();
    let text = prime(repo.path(), &[]);
    let lines = line_names(&text);
    for command in provenance_cli::exposed::EXPOSED {
        if command.declaration().internal {
            continue;
        }
        let form = cli_form(*command);
        assert!(
            lines.contains(&form.as_str()),
            "prime does not name `provenance {form}`, which the CLI shows:\n{text}"
        );
    }
    let forms = code_forms(&text);
    assert!(
        forms.iter().any(|form| form.starts_with("<id> ")),
        "prime gives no CLI form of an action on a record:\n{text}"
    );
    for form in lines.into_iter().chain(forms.iter().map(String::as_str)) {
        assert!(
            accepts(form),
            "the CLI refuses `provenance {form}`, which prime names:\n{text}"
        );
    }
}
