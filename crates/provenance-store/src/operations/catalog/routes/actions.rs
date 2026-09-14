use super::*;

pub(super) fn register(out: &mut Vec<Definition>) {
    computations(out);
    for (name, id, path, backing, description, strip) in [
        (
            "claim-topic",
            "claimTopic",
            "/topics/{id}/claim",
            "claim-topic",
            "Claim one open Topic for an actor.",
            &["scope_id", "id"][..],
        ),
        (
            "release-topic",
            "releaseTopic",
            "/topics/{id}/release",
            "release-topic",
            "Release the claim on one Topic.",
            &["scope_id", "id"][..],
        ),
        (
            "close-topic",
            "closeTopic",
            "/topics/{id}/close",
            "close-topic",
            "Close one Topic in the bound scope.",
            &["scope_id", "id"][..],
        ),
        (
            "claim-question",
            "claimQuestion",
            "/questions/{id}/claim",
            "claim-question",
            "Claim one open Question for an actor.",
            &["scope_id", "id"][..],
        ),
        (
            "release-question",
            "releaseQuestion",
            "/questions/{id}/release",
            "release-question",
            "Release the claim on one Question.",
            &["scope_id", "id"][..],
        ),
        (
            "answer-question",
            "answerQuestion",
            "/questions/{id}/answer",
            "answer-question",
            "Record the answer to one Question.",
            &["scope_id", "id"][..],
        ),
        (
            "submit-requirement-review",
            "submitRequirementReview",
            "/requirements/{id}/submit",
            "submit-requirement-review-v2",
            "Submit the current Requirement revision for review.",
            &["scope_id", "request_id", "requirement_id"][..],
        ),
        (
            "decide-requirement-review",
            "decideRequirementReview",
            "/requirements/{id}/submissions/{proposal_id}/decide",
            "decide-requirement-review-v2",
            "Decide one Requirement review submission.",
            &["scope_id", "request_id", "proposal_id"][..],
        ),
        (
            "withdraw-requirement-review",
            "withdrawRequirementReview",
            "/requirements/{id}/submissions/{proposal_id}/withdraw",
            "withdraw-requirement-review-v2",
            "Withdraw one Requirement review submission.",
            &["scope_id", "request_id", "proposal_id"][..],
        ),
        (
            "begin-verification",
            "beginVerification",
            "/verification-runs/begin-verification",
            "begin-verification",
            "Begin one verification run for a Rule declaration.",
            &[][..],
        ),
        (
            "complete-verification",
            "completeVerification",
            "/verification-runs/{run_id}/complete-verification",
            "complete-verification",
            "Complete one verification run with a passed or failed result.",
            &["run"][..],
        ),
    ] {
        let params = path
            .split('/')
            .filter_map(|p| p.strip_prefix('{').and_then(|p| p.strip_suffix('}')))
            .map(schema::path)
            .collect();
        out.push(backed(
            name,
            id,
            HttpMethod::Post,
            path,
            description,
            backing,
            ResponseKind::Resource,
            strip,
            params,
        ));
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
            &[],
            Vec::new(),
        ));
    }
}
