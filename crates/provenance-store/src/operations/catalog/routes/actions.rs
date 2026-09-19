#![allow(clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog as operation;

pub(super) fn register(out: &mut Vec<Definition>) {
    computations(out);
    out.push(native::<operation::ClaimTopic>(
        "claim-topic",
        "claimTopic",
        "/topics/{id}/claim",
        "Claim one open Topic for an actor.",
    ));
    out.push(native::<operation::ReleaseTopic>(
        "release-topic",
        "releaseTopic",
        "/topics/{id}/release",
        "Release the claim on one Topic.",
    ));
    out.push(native::<operation::CloseTopic>(
        "close-topic",
        "closeTopic",
        "/topics/{id}/close",
        "Close one Topic in the bound scope.",
    ));
    out.push(native::<operation::ClaimQuestion>(
        "claim-question",
        "claimQuestion",
        "/questions/{id}/claim",
        "Claim one open Question for an actor.",
    ));
    out.push(native::<operation::ReleaseQuestion>(
        "release-question",
        "releaseQuestion",
        "/questions/{id}/release",
        "Release the claim on one Question.",
    ));
    out.push(native::<operation::AnswerQuestion>(
        "answer-question",
        "answerQuestion",
        "/questions/{id}/answer",
        "Record the answer to one Question.",
    ));
    out.push(
        backed::<operation::SubmitRequirementReviewV2>(
            "submit-requirement-review",
            "submitRequirementReview",
            HttpMethod::Post,
            "/requirements/{id}/submit",
            "Submit the current Requirement revision for review.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .path_field("id", "requirement_id")
        .scope("scope_id")
        .header("Idempotency-Key", "request_id", false),
    );
    out.push(
        backed::<operation::DecideRequirementReviewV2>(
            "decide-requirement-review",
            "decideRequirementReview",
            HttpMethod::Post,
            "/requirements/{id}/submissions/{proposal_id}/decide",
            "Decide one Requirement review submission.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("proposal_id")],
        )
        .path_field("id", "requirement_id")
        .scope("scope_id")
        .header("Idempotency-Key", "request_id", false),
    );
    out.push(
        backed::<operation::WithdrawRequirementReviewV2>(
            "withdraw-requirement-review",
            "withdrawRequirementReview",
            HttpMethod::Post,
            "/requirements/{id}/submissions/{proposal_id}/withdraw",
            "Withdraw one Requirement review submission.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("proposal_id")],
        )
        .path_field("id", "requirement_id")
        .scope("scope_id")
        .header("Idempotency-Key", "request_id", false),
    );
    out.push(backed::<operation::BeginVerification>(
        "begin-verification",
        "beginVerification",
        HttpMethod::Post,
        "/verification-runs/begin-verification",
        "Begin one verification run for a Rule declaration.",
        ResponseKind::Resource,
        Vec::new(),
    ));
    out.push(
        backed::<operation::CompleteVerification>(
            "complete-verification",
            "completeVerification",
            HttpMethod::Post,
            "/verification-runs/{run_id}/complete-verification",
            "Complete one verification run with a passed or failed result.",
            ResponseKind::Resource,
            vec![schema::path("run_id")],
        )
        .path_field("run_id", "run"),
    );
}

fn native<O: Operation>(
    name: &'static str,
    operation_id: &'static str,
    path: &'static str,
    description: &'static str,
) -> Definition {
    backed::<O>(
        name,
        operation_id,
        HttpMethod::Post,
        path,
        description,
        ResponseKind::Resource,
        vec![schema::path("id")],
    )
    .scope("scope_id")
}

fn computations(out: &mut Vec<Definition>) {
    out.push(backed::<operation::CheckStatement>(
        "check-statement",
        "checkStatement",
        HttpMethod::Post,
        "/statement-checks",
        "Check one statement against the configured writing standard.",
        ResponseKind::Result,
        Vec::new(),
    ));
    out.push(backed::<operation::Plan>(
        "plan-authoring",
        "planAuthoring",
        HttpMethod::Post,
        "/authoring-plans",
        "Plan the deterministic changes for one typed authoring document.",
        ResponseKind::Result,
        Vec::new(),
    ));
    out.push(backed::<operation::Apply>(
        "apply-authoring",
        "applyAuthoring",
        HttpMethod::Post,
        "/authoring-changes",
        "Apply one typed authoring document to the bound scope.",
        ResponseKind::Result,
        Vec::new(),
    ));
}
