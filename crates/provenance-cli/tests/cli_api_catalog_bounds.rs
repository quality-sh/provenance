//! The CLI api catalog output: the bounds in the JSON answer, and the route count, the
//! default limit of 50 and the "see more" line in the text answer.

use provenance_macros::verifies;
use serde_json::{json, Value};

fn binary() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    assert_cmd::Command::new(binary())
        .args(["init", "--path", &repo, "--scope", "default"])
        .assert()
        .success();
    (directory, repo)
}

fn stdout(repo: &str, flags: &[&str]) -> String {
    let output = assert_cmd::Command::new(binary())
        .args(["api", "--repo", repo])
        .args(flags)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{flags:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn bounds(repo: &str, flags: &[&str]) -> Value {
    let mut all = vec!["--format", "json"];
    all.extend(flags);
    let answer: Value = serde_json::from_str(&stdout(repo, &all)).unwrap();
    assert_eq!(
        answer["routes"].as_array().unwrap().len() as u64,
        answer["bounds"]["returned"].as_u64().unwrap()
    );
    answer["bounds"].clone()
}

const MORE: &str = "Use --filter <text> or --limit <number> to see more.";
const SCHEMAS: &str =
    "Use --filter \"<method> <path>\" --format json to see the schemas of one route.";

/// The route count line, the number of route lines, and the lines after the routes.
fn text(repo: &str, flags: &[&str]) -> (String, usize, Vec<String>) {
    let printed = stdout(repo, flags);
    let lines = printed.lines().collect::<Vec<_>>();
    let routes = lines.iter().filter(|line| line.starts_with("- ")).count();
    let tail = lines
        .iter()
        .rev()
        .take_while(|line| line.starts_with("Use "))
        .map(|line| (*line).to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (lines[0].to_owned(), routes, tail)
}

#[test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
fn json_catalog_reports_its_bounds_and_has_no_default_limit() {
    let (_directory, repo) = init();
    let all = bounds(&repo, &[]);
    let total = all["total"].as_u64().unwrap();
    assert!(total > 100, "{all}");
    assert_eq!(
        all,
        json!({"total": total, "matching": total, "returned": total, "truncated": false})
    );
    let matching = bounds(&repo, &["--filter", "withdraw"])["matching"]
        .as_u64()
        .unwrap();
    assert!(matching > 2 && matching < 50, "{matching}");
    assert_eq!(
        bounds(&repo, &["--filter", "withdraw", "--limit", "2"]),
        json!({"total": total, "matching": matching, "returned": 2, "truncated": true})
    );
}

#[test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
fn text_catalog_gives_at_most_50_routes_and_says_how_to_see_more() {
    let (_directory, repo) = init();
    let more = vec![MORE.to_owned(), SCHEMAS.to_owned()];
    let complete = vec![SCHEMAS.to_owned()];
    assert_eq!(
        text(&repo, &[]),
        ("api routes: 50".to_owned(), 50, more.clone())
    );
    assert_eq!(
        text(&repo, &["--filter", "/"]),
        ("api routes: 50".to_owned(), 50, more.clone())
    );
    assert_eq!(
        text(&repo, &["--limit", "100"]),
        ("api routes: 100".to_owned(), 100, more.clone())
    );
    assert_eq!(
        text(&repo, &["--filter", "withdraw", "--limit", "2"]),
        ("api routes: 2".to_owned(), 2, more)
    );
    let (count, routes, tail) = text(&repo, &["--filter", "withdraw"]);
    assert!(routes > 2 && routes < 50, "{routes}");
    assert_eq!(
        (count, tail),
        (format!("api routes: {routes}"), complete.clone())
    );
    let total = bounds(&repo, &[])["total"].as_u64().unwrap();
    let limit = total.to_string();
    let (_, routes, tail) = text(&repo, &["--limit", &limit]);
    assert_eq!((routes as u64, tail), (total, complete));
}
