use super::{
    directory_file, envelope, init, json as read_json, output, provenance, refusal, seed_source,
};
use serde_json::{json, Value};
use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn api_rejects_unsupported_methods_and_malformed_flags_before_any_call() {
    let (_directory, repo) = init();

    provenance()
        .args(["api", "sources", "--repo", &repo, "--method", "delete"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid value"));

    provenance()
        .args(["api", "sources", "--repo", &repo, "--header", "NoSeparator"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("NAME: VALUE"));

    provenance()
        .args(["api", "requirements", "--repo", &repo, "--query", "limit"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("NAME=VALUE"));

    provenance()
        .args([
            "api",
            "requirements",
            "--repo",
            &repo,
            "--method",
            "get",
            "--method",
            "post",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot be used multiple times"));

    let body = directory_file(
        &repo,
        "body.json",
        json!({"name": "X"}).to_string().as_str(),
    );
    let refused = refusal(&[
        "api", "sources", "--repo", &repo, "--method", "get", "--input", &body,
    ]);
    assert_eq!(refused["error"]["kind"], "invalid_input");
    assert_eq!(refused["error"]["field"], "input");
}

/// A repeated header refuses in the text format with the JSON envelope and
/// nothing after it, so a hint on a header refusal fails the test. A repeated
/// query with no `query` selector on a route that has forms reads the first
/// schema of the name: `limit` takes one value, so the hint has no comma
/// example.
#[test]
fn api_refuses_repeated_header_and_query_names() {
    let (_directory, repo) = init();

    for (first, second) in [
        ("Accept: one", "Accept: two"),
        ("If-Match: \"1\"", "if-match: \"2\""),
    ] {
        let text = output(&[
            "api",
            "requirements",
            "--repo",
            &repo,
            "--header",
            first,
            "--header",
            second,
        ]);
        assert!(!text.status.success(), "{first}");
        assert_eq!(
            String::from_utf8_lossy(&text.stderr).trim_end(),
            "Error: {\"error\":{\"field\":\"header\",\"kind\":\"invalid_input\",\
             \"reason\":\"invalid_value\"},\"meta\":{}}",
            "{first}"
        );
    }

    let refused = refusal(&[
        "api",
        "requirements",
        "--repo",
        &repo,
        "--format",
        "json",
        "--query",
        "limit=1",
        "--query",
        "limit=2",
    ]);
    assert_eq!(
        refused,
        json!({"error": {"field": "query",
            "hint": "--query limit occurs more than one time; give each parameter one time",
            "kind": "invalid_input", "reason": "invalid_value"}, "meta": {}})
    );
}

#[test]
fn api_method_without_a_path_refuses_instead_of_discovering() {
    let (_directory, repo) = init();

    let refused = refusal(&["api", "--method", "post", "--repo", &repo]);
    assert_eq!(refused["error"]["kind"], "invalid_input");
    assert_eq!(refused["error"]["field"], "method");
}

/// Run one api command with standard input held open and never written,
/// and return its refusal; a command that waits on the input fails the test.
fn refusal_with_open_stdin(arguments: &[&str]) -> Value {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let held = child.stdin.take();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("api waited on standard input: {arguments:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(held);
    assert!(!status.success(), "{arguments:?}");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    envelope(&stderr)
}

#[test]
fn api_refuses_a_stdin_body_before_reading_it() {
    let (_directory, repo) = init();

    let repo = repo.as_str();
    for arguments in [
        vec!["api", "--input", "-", "--repo", repo],
        vec!["api", "sources", "--input", "-", "--repo", repo],
        vec![
            "api", "sources", "--method", "GET", "--input", "-", "--repo", repo,
        ],
    ] {
        let refused = refusal_with_open_stdin(&arguments);
        assert_eq!(refused["error"]["kind"], "invalid_input", "{arguments:?}");
        assert_eq!(refused["error"]["field"], "input", "{arguments:?}");
    }
}

const LIST_HINT: &str = "--query relations occurs more than one time; give each parameter one \
     time and put the items of a list in one comma-separated value, for example \
     --query relations=a,b";
const ONE_VALUE_HINT: &str =
    "--query query occurs more than one time; give each parameter one time";

/// Implementation-aid test: no active Rule states this refusal yet. It pins
/// that a repeated list parameter names the parameter and the comma form, in
/// the JSON format and the text format, and that the named form then reads
/// the record.
#[test]
fn api_repeated_query_names_the_parameter_and_the_comma_form() {
    let (_directory, repo) = init();
    seed_source(&repo);
    let repeated = [
        "api",
        "sources/source_api",
        "--repo",
        &repo,
        "--query",
        "query=neighbors",
        "--query",
        "relations=cites",
        "--query",
        "relations=refines",
    ];

    let refused = refusal(&[&repeated[..], &["--format", "json"]].concat());
    assert_eq!(
        refused,
        json!({"error": {"field": "query", "hint": LIST_HINT, "kind": "invalid_input",
            "reason": "invalid_value"}, "meta": {}})
    );
    let text = output(&repeated);
    assert!(!text.status.success());
    assert_eq!(
        String::from_utf8_lossy(&text.stderr).trim_end(),
        format!(
            "Error: {{\"error\":{{\"field\":\"query\",\"kind\":\"invalid_input\",\
             \"reason\":\"invalid_value\"}},\"meta\":{{}}}}; {LIST_HINT}"
        )
    );

    let read = read_json(&[
        "api",
        "sources/source_api",
        "--repo",
        &repo,
        "--format",
        "json",
        "--query",
        "query=neighbors",
        "--query",
        "relations=cites,refines",
    ]);
    assert_eq!(read["data"]["id"], "source_api");
}

/// Implementation-aid test: no active Rule states this refusal yet. A
/// parameter that takes one value refuses the comma form, so the hint gives
/// no comma example, and one use of the parameter then reads the record.
#[test]
fn api_repeated_one_value_query_gives_no_comma_example() {
    let (_directory, repo) = init();
    seed_source(&repo);

    let refused = refusal(&[
        "api",
        "sources/source_api",
        "--repo",
        &repo,
        "--format",
        "json",
        "--query",
        "query=neighbors",
        "--query",
        "query=stale",
    ]);
    assert_eq!(
        refused,
        json!({"error": {"field": "query", "hint": ONE_VALUE_HINT, "kind": "invalid_input",
            "reason": "invalid_value"}, "meta": {}})
    );

    let read = read_json(&[
        "api",
        "sources/source_api",
        "--repo",
        &repo,
        "--format",
        "json",
        "--query",
        "query=neighbors",
    ]);
    assert_eq!(read["data"]["id"], "source_api");
}

/// Implementation-aid test: no active Rule states this refusal yet. A
/// repeated header wins over a repeated query, and its refusal has no query
/// hint.
#[test]
fn api_repeated_header_and_query_give_the_header_refusal_with_no_hint() {
    let (_directory, repo) = init();

    let refused = refusal(&[
        "api", "sources", "--repo", &repo, "--format", "json", "--header", "X: 1", "--header",
        "X: 2", "--query", "a=1", "--query", "a=2",
    ]);
    assert_eq!(
        refused,
        json!({"error": {"field": "header", "kind": "invalid_input",
            "reason": "invalid_value"}, "meta": {}})
    );
}

/// Implementation-aid test: no active Rule states this refusal yet. With no
/// path, or with a path that no route matches, one use of the parameter also
/// fails, so the refusal gives no hint.
#[test]
fn api_repeated_query_with_no_route_gives_no_hint() {
    let (_directory, repo) = init();

    for path in [None, Some("nowhere/at_all")] {
        let mut arguments = vec!["api"];
        arguments.extend(path);
        arguments.extend([
            "--repo",
            repo.as_str(),
            "--format",
            "json",
            "--query",
            "a=1",
            "--query",
            "a=2",
        ]);
        assert_eq!(
            refusal(&arguments),
            json!({"error": {"field": "query", "kind": "invalid_input",
                "reason": "invalid_value"}, "meta": {}}),
            "{path:?}"
        );
    }
}
