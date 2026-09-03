use assert_cmd::Command;
use predicates::str::contains;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;

const STORED_FAMILIES: [(&str, &str); 17] = [
    ("source", "scopes/default/sources/source.jsonl"),
    ("domain", "scopes/default/domains/domain.jsonl"),
    ("requirement", "scopes/default/requirements/req.jsonl"),
    ("boundary", "scopes/default/boundaries/boundary.jsonl"),
    ("topic", "scopes/default/topics/topic.jsonl"),
    ("question", "scopes/default/questions/question.jsonl"),
    ("resolution", "scopes/default/resolutions/res.jsonl"),
    ("rule", "scopes/default/rules/rule.jsonl"),
    ("edge", "edges/edges-00.jsonl"),
    ("thread", "scopes/default/threads/threads.jsonl"),
    ("message", "scopes/default/threads/2026-07.jsonl"),
    (
        "contribution",
        "scopes/default/ideation/contributions.jsonl",
    ),
    (
        "synthesis",
        "scopes/default/ideation/synthesis_packets.jsonl",
    ),
    ("proposal", "scopes/default/ideation/proposal_cards.jsonl"),
    ("assertion", "scopes/default/ideation/assertions.jsonl"),
    ("disposition", "scopes/default/ideation/dispositions.jsonl"),
    (
        "legacy_disposition",
        "scopes/default/ideation/promotion_decisions.jsonl",
    ),
];

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn wiki_and_check_refuse_v2_rows_in_every_stored_family() {
    for command in ["check", "wiki"] {
        for (family, relative_path) in STORED_FAMILIES {
            let dir = tempfile::tempdir().unwrap();
            let repo = dir.path().to_str().unwrap();
            init(repo);
            let record_id = format!("{family}_future");
            plant_v2_row(dir.path(), relative_path, &record_id);

            let mut invocation = Command::cargo_bin("provenance").unwrap();
            if command == "check" {
                invocation.args(["check", "--repo", repo]);
            } else {
                invocation.args([
                    "wiki",
                    "build",
                    "--repo",
                    repo,
                    "--out",
                    dir.path().join("wiki").to_str().unwrap(),
                ]);
            }
            invocation
                .assert()
                .failure()
                // Windows spells ancestor components with backslashes, and
                // joins can mix separators; compare slash-normalized text.
                .stderr(predicates::function::function(move |text: &str| {
                    text.replace('\\', "/").contains(relative_path)
                }))
                .stderr(contains(format!("record {record_id}")))
                .stderr(contains(format!(
                    "has schema_version {}, but this build reads schema_version {} only",
                    SUPPORTED_SCHEMA_VERSION.0 + 1,
                    SUPPORTED_SCHEMA_VERSION.0
                )));
        }
    }
}

#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn coverage_rule_validation_refuses_a_v2_rule_row() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    init(repo);
    let source_dir = dir.path().join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("lib.rs"), "fn example() {}\n").unwrap();
    plant_v2_row(dir.path(), "scopes/default/rules/rule.jsonl", "rule_future");

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo,
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--validate-rules",
        ])
        .assert()
        .failure()
        .stderr(predicates::function::function(|text: &str| {
            text.replace('\\', "/")
                .contains("scopes/default/rules/rule.jsonl line 1")
        }))
        .stderr(contains("record rule_future"))
        .stderr(contains(format!(
            "has schema_version {}, but this build reads schema_version {} only",
            SUPPORTED_SCHEMA_VERSION.0 + 1,
            SUPPORTED_SCHEMA_VERSION.0
        )));
}

/// A hand-edited record does not load, and the refusal says where it is.
///
/// The version guard sits on the store's read path, so it covers every family
/// the store reads rather than the ideation ones the aggregate validator
/// judges. A requirement is the plainest case: it is not an ideation record,
/// and nothing but the read guard would have stopped it. Both a read command
/// and `check` are run, because the point of guarding the read is that no
/// command gets to see the record.
#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn a_hand_edited_requirement_version_is_refused_by_every_reader() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap().to_string();
    init(&repo);
    create_requirement(
        &repo,
        "req_overtime",
        "Overtime must follow the award thresholds",
    );
    let path = dir
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let stored = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        stored.replace(
            &format!("\"schema_version\":{}", SUPPORTED_SCHEMA_VERSION.0),
            &format!("\"schema_version\":{}", SUPPORTED_SCHEMA_VERSION.0 + 1),
        ),
    )
    .unwrap();

    let export = dir.path().join("export.json");
    for command in [
        vec!["check", "--repo", repo.as_str()],
        vec![
            "export",
            "--repo",
            repo.as_str(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            export.to_str().unwrap(),
        ],
    ] {
        Command::cargo_bin("provenance")
            .unwrap()
            .args(&command)
            .assert()
            .failure()
            .stderr(contains("requirements/req.jsonl line 1"))
            .stderr(contains("record req_overtime"))
            .stderr(contains(format!(
                "has schema_version {}, but this build reads schema_version {} only",
                SUPPORTED_SCHEMA_VERSION.0 + 1,
                SUPPORTED_SCHEMA_VERSION.0
            )));
    }
}

/// Writing to a shard that holds a hand-edited record changes nothing.
///
/// A write reads the shard first and writes all of it back, so an unguarded
/// write was worse than an unguarded read: `requirements create` for an
/// unrelated id used to succeed, re-serialise the version-2 neighbour from
/// whatever fields the current struct still recognised, and drop the rest -
/// laundering into the supported layout exactly the record every reader
/// refuses. The shard is compared byte for byte because "the command failed"
/// is not the claim; the claim is that the file on disk was not touched.
#[test]
#[verifies("rule_reads_supported_version_only", examples)]
fn a_write_beside_a_hand_edited_record_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap().to_string();
    init(&repo);
    create_requirement(
        &repo,
        "req_overtime",
        "Overtime must follow the award thresholds",
    );
    let path = dir
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let planted = std::fs::read_to_string(&path)
        .unwrap()
        .replace(
            &format!("\"schema_version\":{}", SUPPORTED_SCHEMA_VERSION.0),
            &format!("\"schema_version\":{} ", SUPPORTED_SCHEMA_VERSION.0 + 1),
        )
        .replace(
            "\"statement\"",
            "\"unknown_to_this_build\":\"keep me\",\"statement\"",
        );
    std::fs::write(&path, &planted).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.as_str(),
            "--scope",
            "default",
            "--id",
            "req_penalty_rates",
            "--statement",
            "Penalty rates apply on public holidays",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(contains("requirements/req.jsonl line 1"))
        .stderr(contains("record req_overtime"))
        .stderr(contains(format!(
            "has schema_version {}, but this build reads schema_version {} only",
            SUPPORTED_SCHEMA_VERSION.0 + 1,
            SUPPORTED_SCHEMA_VERSION.0
        )));

    assert_eq!(std::fs::read_to_string(&path).unwrap(), planted);
}

fn init(repo: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--path", repo, "--scope", "default"])
        .assert()
        .success();
}

fn create_requirement(repo: &str, id: &str, statement: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            id,
            "--statement",
            statement,
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn plant_v2_row(repo: &std::path::Path, relative_path: &str, record_id: &str) {
    let path = repo.join(".provenance/state").join(relative_path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "{{\"schema_version\":{},\"id\":\"{record_id}\"}}\n",
            SUPPORTED_SCHEMA_VERSION.0 + 1
        ),
    )
    .unwrap();
}
