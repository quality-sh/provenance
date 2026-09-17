#![allow(
    clippy::literal_string_with_formatting_args,
    clippy::too_many_lines,
    clippy::wildcard_imports
)]

use super::*;

pub(super) fn register(out: &mut Vec<Definition>) {
    computations(out);
    for (name, id, path, backing, description) in [
        (
            "claim-topic",
            "claimTopic",
            "/topics/{id}/claim",
            "claim-topic",
            "Claim one open Topic for an actor.",
        ),
        (
            "release-topic",
            "releaseTopic",
            "/topics/{id}/release",
            "release-topic",
            "Release the claim on one Topic.",
        ),
        (
            "close-topic",
            "closeTopic",
            "/topics/{id}/close",
            "close-topic",
            "Close one Topic in the bound scope.",
        ),
        (
            "claim-question",
            "claimQuestion",
            "/questions/{id}/claim",
            "claim-question",
            "Claim one open Question for an actor.",
        ),
        (
            "release-question",
            "releaseQuestion",
            "/questions/{id}/release",
            "release-question",
            "Release the claim on one Question.",
        ),
        (
            "answer-question",
            "answerQuestion",
            "/questions/{id}/answer",
            "answer-question",
            "Record the answer to one Question.",
        ),
        (
            "submit-requirement-review",
            "submitRequirementReview",
            "/requirements/{id}/submit",
            "submit-requirement-review-v2",
            "Submit the current Requirement revision for review.",
        ),
        (
            "decide-requirement-review",
            "decideRequirementReview",
            "/requirements/{id}/submissions/{proposal_id}/decide",
            "decide-requirement-review-v2",
            "Decide one Requirement review submission.",
        ),
        (
            "withdraw-requirement-review",
            "withdrawRequirementReview",
            "/requirements/{id}/submissions/{proposal_id}/withdraw",
            "withdraw-requirement-review-v2",
            "Withdraw one Requirement review submission.",
        ),
        (
            "begin-verification",
            "beginVerification",
            "/verification-runs/begin-verification",
            "begin-verification",
            "Begin one verification run for a Rule declaration.",
        ),
        (
            "complete-verification",
            "completeVerification",
            "/verification-runs/{run_id}/complete-verification",
            "complete-verification",
            "Complete one verification run with a passed or failed result.",
        ),
    ] {
        let params = path
            .split('/')
            .filter_map(|p| p.strip_prefix('{').and_then(|p| p.strip_suffix('}')))
            .map(schema::path)
            .collect();
        let mut definition = backed(
            name,
            id,
            HttpMethod::Post,
            path,
            description,
            backing,
            ResponseKind::Resource,
            params,
        );
        if matches!(
            name,
            "submit-requirement-review"
                | "decide-requirement-review"
                | "withdraw-requirement-review"
        ) {
            definition = definition.path_field("id", "requirement_id").header(
                "Idempotency-Key",
                "request_id",
                false,
            );
        }
        if name == "complete-verification" {
            definition = definition.path_field("run_id", "run");
        }
        out.push(definition);
    }
}

fn computations(out: &mut Vec<Definition>) {
    for (name, id, path, backing, description) in [
        (
            "check-statement",
            "checkStatement",
            "/statement-checks",
            "check-statement",
            "Check one statement against the configured writing standard.",
        ),
        (
            "plan-authoring",
            "planAuthoring",
            "/authoring-plans",
            "plan",
            "Plan the deterministic changes for one typed authoring document.",
        ),
        (
            "apply-authoring",
            "applyAuthoring",
            "/authoring-changes",
            "apply",
            "Apply one typed authoring document to the bound scope.",
        ),
    ] {
        out.push(backed(
            name,
            id,
            HttpMethod::Post,
            path,
            description,
            backing,
            ResponseKind::Result,
            Vec::new(),
        ));
    }
}
