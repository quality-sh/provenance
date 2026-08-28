//! Pins the agent-facing guidance that keeps an audit off the wrong evidence.
//!
//! The guidance is what an agent reads before it shapes or audits a graph, so
//! these are the sentences that decide whether a planned Rule gets reported as
//! invented. They are read out of the binary, which is the copy an installed
//! agent actually gets, rather than out of a file in this checkout.

use assert_cmd::Command;

fn skill(name: &str) -> String {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["skills", "show", name])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

/// An audit weighs a Rule against sources, Requirements, and ratified
/// decisions. Those three are named, and the two fidelities are kept apart.
#[test]
fn shaping_guidance_audits_a_rule_against_its_grounding_and_not_against_code() {
    let shaping = skill("provenance-shaping");

    assert!(shaping.contains("## Auditing a Rule"), "{shaping}");
    assert!(shaping.contains("Decision fidelity"), "{shaping}");
    assert!(shaping.contains("Implementation fidelity"), "{shaping}");
    assert!(
        shaping.contains(
            "Audit the Rule against its\nsources, the Requirement it refines, and the \
             ratified decisions that produced it."
        ),
        "{shaping}"
    );
    assert!(
        shaping.contains(
            "**Never report a Rule as invented, invalid, or unsupported because no code \
             implements it.**"
        ),
        "{shaping}"
    );
}

/// Unsupported agent-authored behaviour keeps a state a human can still
/// refuse. Promoting it to `active` to clear a report is named and refused.
#[test]
fn shaping_guidance_keeps_unsupported_behaviour_out_of_active() {
    let shaping = skill("provenance-shaping");

    for state in [
        "`draft`",
        "`review`",
        "`proposed` proposal",
        "open Question",
    ] {
        assert!(
            shaping.contains(state),
            "shaping guidance does not name {state}: {shaping}"
        );
    }
    assert!(
        shaping.contains("Do not make it `active`\nto clear a report."),
        "{shaping}"
    );
}

/// The citation fields are citations. The guidance says so, and no example
/// shows a code path in one of them.
#[test]
fn guidance_calls_the_source_fields_citations_and_not_planned_code_homes() {
    for name in ["provenance-shaping", "provenance-grounded-writing"] {
        let text = skill(name);
        assert!(
            text.contains("are not a planned home for the code")
                || text.contains("They name no home for the code"),
            "{name} does not deny the planned-home reading: {text}"
        );
    }

    let writing = skill("provenance-grounded-writing");
    assert!(
        !writing.contains("--source-document ApprovalService.php"),
        "the worked example still cites a code file as source material: {writing}"
    );
    assert!(
        writing.contains("--source-document \"Finance Policy v3 (2026 revision)\""),
        "{writing}"
    );
}

/// A Requirement alone anchors a Rule, and the obsolete binary that demanded a
/// Resolution producer is called out with the command that replaces it.
#[test]
fn shaping_guidance_tells_an_agent_to_check_the_installed_cli() {
    let shaping = skill("provenance-shaping");

    assert!(shaping.contains("## Check your CLI"), "{shaping}");
    assert!(shaping.contains("provenance --version"), "{shaping}");
    assert!(
        shaping.contains("cargo install provenance-cli --force"),
        "{shaping}"
    );
    assert!(
        shaping.contains("A Resolution producer is not required"),
        "{shaping}"
    );
}
