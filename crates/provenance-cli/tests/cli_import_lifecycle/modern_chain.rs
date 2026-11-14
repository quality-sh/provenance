use super::modern_lifecycle::add_modern_lifecycle;
use super::support::{create_system_source, export_scope, import_scope, init_repo, write_json};
use provenance_macros::verifies;
use std::path::PathBuf;

#[test]
fn import_cannot_omit_existing_modern_lifecycle_chain() {
    let fixture = ModernLifecycleFixture::new();
    let mut missing_assertion = fixture.lifecycle.clone();
    missing_assertion["assertion_records"] = serde_json::json!([]);
    missing_assertion["dispositions"] = serde_json::json!([]);
    fixture
        .import_value("missing-assertion.json", &missing_assertion)
        .failure()
        .stderr(predicates::str::contains(
            "qualifying proposal proposal_a requires an assertion",
        ));

    fixture.import_lifecycle().success();
    fixture.assert_asserted_evidence_is_immutable();

    let mut omitted = fixture.lifecycle.clone();
    omitted["proposal_cards"] = serde_json::json!([]);
    omitted["assertion_records"] = serde_json::json!([]);
    omitted["dispositions"] = serde_json::json!([]);
    fixture
        .import_value("omitted.json", &omitted)
        .failure()
        .stderr(predicates::str::contains("immutable proposal proposal_a"));
}

#[test]
fn import_rejects_duplicate_evidence_record_ids() {
    let fixture = ModernLifecycleFixture::new();
    let cases = [
        (
            "contributions",
            "strongest_finding",
            "duplicate immutable contribution id contribution_a",
        ),
        (
            "synthesis_packets",
            "summary",
            "duplicate immutable synthesis packet id synthesis_a",
        ),
    ];

    for (records, divergent_field, expected) in cases {
        let mut duplicate = fixture.lifecycle.clone();
        let mut divergent = duplicate[records][0].clone();
        divergent[divergent_field] = serde_json::json!("Divergent");
        duplicate[records].as_array_mut().unwrap().push(divergent);

        fixture
            .import_value(&format!("duplicate-{records}.json"), &duplicate)
            .failure()
            .stderr(predicates::str::contains(expected));
    }
}

#[test]
fn import_accepts_rejected_or_deferred_disposition_after_assertion() {
    for decision in ["rejected", "deferred"] {
        let fixture = ModernLifecycleFixture::new();
        let mut lifecycle = fixture.lifecycle.clone();
        lifecycle["dispositions"][0]["decision"] = serde_json::json!(decision);
        fixture
            .import_value(&format!("{decision}-asserted.json"), &lifecycle)
            .success();
    }
}

#[test]
fn import_and_export_round_trip_generic_external_action() {
    let fixture = ModernLifecycleFixture::new();
    fixture.import_lifecycle().success();
    let exported = fixture.root.join("round-trip.json");
    export_scope(&fixture.repo, &exported).success();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(exported).unwrap()).unwrap();

    assert_eq!(
        value["dispositions"][0]["external_action"],
        fixture.lifecycle["dispositions"][0]["external_action"]
    );
}

struct ModernLifecycleFixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
    lifecycle: serde_json::Value,
}

impl ModernLifecycleFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let repo = root.join("repo");
        init_repo(&repo, Some("reviewer"));
        create_system_source(&repo);
        let baseline = root.join("baseline.json");
        export_scope(&repo, &baseline).success();
        let mut lifecycle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&baseline).unwrap()).unwrap();
        add_modern_lifecycle(&mut lifecycle);
        Self {
            _directory: directory,
            root,
            repo,
            lifecycle,
        }
    }

    fn import_lifecycle(&self) -> assert_cmd::assert::Assert {
        self.import_value("lifecycle.json", &self.lifecycle)
    }

    fn import_value(&self, name: &str, value: &serde_json::Value) -> assert_cmd::assert::Assert {
        let input = self.root.join(name);
        write_json(&input, value);
        import_scope(&self.repo, &input)
    }

    fn assert_asserted_evidence_is_immutable(&self) {
        let mut changed_contribution = self.lifecycle.clone();
        changed_contribution["contributions"][0]["strongest_finding"] =
            serde_json::json!("Rewritten");
        self.import_value("changed-contribution.json", &changed_contribution)
            .failure()
            .stderr(predicates::str::contains(
                "contribution contribution_a is referenced by an assertion and cannot be replaced",
            ));

        // The stored contribution is not in the incoming scope at all, so it
        // is dropped rather than replaced.
        let mut retargeted_claim = self.lifecycle.clone();
        retargeted_claim["contributions"][0]["id"] = serde_json::json!("contribution_b");
        self.import_value("retargeted-claim.json", &retargeted_claim)
            .failure()
            .stderr(predicates::str::contains(
                "contribution contribution_a is referenced by an assertion and cannot be deleted",
            ));

        let mut changed_synthesis = self.lifecycle.clone();
        changed_synthesis["synthesis_packets"][0]["summary"] = serde_json::json!("Rewritten");
        self.import_value("changed-synthesis.json", &changed_synthesis)
            .failure()
            .stderr(predicates::str::contains(
                "synthesis packet synthesis_a is referenced by an assertion and cannot be replaced",
            ));

        // Dropping the packet has no import that reaches it: the assertion
        // naming the packet is immutable, so an import that drops the packet
        // either keeps the assertion and fails the aggregate, or drops it and
        // fails the immutable-record check first. The deletion rule stands
        // behind both; `handlers::import`'s unit test walks it for both kinds.
        let mut dropped_synthesis = self.lifecycle.clone();
        dropped_synthesis["synthesis_packets"] = serde_json::json!([]);
        self.import_value("dropped-synthesis.json", &dropped_synthesis)
            .failure()
            .stderr(predicates::str::contains(
                "assertion synthesis packet does not exist",
            ));
    }
}

/// The exit code, the standard output and the standard error of a command.
fn answer(output: &std::process::Output) -> (Option<i32>, String, String) {
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

impl ModernLifecycleFixture {
    /// The fixture after an import of its lifecycle, with the row of
    /// `proposal_a` on two lines of `proposal_cards.jsonl`. The result also
    /// gives the path of the file and its one row.
    fn with_a_proposal_id_on_two_lines() -> (Self, PathBuf, String) {
        let fixture = Self::new();
        fixture.import_lifecycle().success();
        let cards = fixture
            .repo
            .join(".provenance/state/scopes/default/ideation/proposal_cards.jsonl");
        let row = std::fs::read_to_string(&cards).unwrap();
        assert_eq!(row.lines().count(), 1, "{row}");
        std::fs::write(&cards, format!("{row}{row}")).unwrap();
        (fixture, cards, row)
    }
}

/// The fault of `proposal_a` on the lines 1 and 2 of `proposal_cards.jsonl`.
const DUPLICATE_PROPOSAL: &str = "scopes/default/ideation/proposal_cards.jsonl line 2: \
    duplicate immutable proposal id proposal_a at line 1 and at line 2";

/// `provenance check` over a scope with `proposal_a` on two lines of
/// `proposal_cards.jsonl` fails and gives the fault at the second line, with
/// a message that names the two lines.
#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn check_names_the_two_lines_of_a_proposal_id() {
    let (fixture, _, _) = ModernLifecycleFixture::with_a_proposal_id_on_two_lines();
    let check = super::support::provenance()
        .args(["check", "--repo", fixture.repo.to_str().unwrap()])
        .output()
        .unwrap();
    let (code, stdout, stderr) = answer(&check);
    let finding = format!("  - {DUPLICATE_PROPOSAL}");
    assert_eq!(
        (code, stdout.lines().any(|line| line == finding)),
        (Some(1), true),
        "{stdout}{stderr}"
    );
}

/// `provenance export` over a scope with `proposal_a` on two lines of
/// `proposal_cards.jsonl` refuses with the fault at the second line, with a
/// message that names the two lines, and writes no export.
#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn export_names_the_two_lines_of_a_proposal_id() {
    let (fixture, _, _) = ModernLifecycleFixture::with_a_proposal_id_on_two_lines();
    let output = fixture.root.join("export.json");
    let export = super::support::provenance()
        .args(["export", "--repo", fixture.repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "json", "--output"])
        .arg(&output)
        .output()
        .unwrap();
    let (code, stdout, stderr) = answer(&export);
    let refusal = format!("Error: {DUPLICATE_PROPOSAL}");
    assert_eq!(
        (
            code,
            stderr.lines().any(|line| line == refusal),
            output.exists()
        ),
        (Some(1), true, false),
        "{stdout}{stderr}"
    );
}

/// An import of a valid export replaces a scope with `proposal_a` on two
/// lines of `proposal_cards.jsonl`, and the id is on one line after it.
///
/// No Rule: an implementation aid. It pins that an import of a valid export
/// repairs a scope that has an id on two lines.
#[test]
fn an_import_replaces_a_scope_with_a_proposal_id_on_two_lines() {
    let (fixture, cards, row) = ModernLifecycleFixture::with_a_proposal_id_on_two_lines();
    let import = fixture
        .import_value("repair.json", &fixture.lifecycle)
        .get_output()
        .clone();
    let imported = (
        Some(0),
        "{\n  \"status\": \"ok\",\n  \"dry_run\": false,\n  \"records\": 6\n}\n".to_owned(),
        String::new(),
    );
    assert_eq!(
        (answer(&import), std::fs::read_to_string(&cards).unwrap()),
        (imported, row)
    );
}
