#![allow(clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog as operation;

pub(super) fn register(out: &mut Vec<Definition>) {
    computations(out);
    topic_actions(out);
    question_actions(out);
    review_actions(out);
    verification_runs(out);
}

fn topic_actions(out: &mut Vec<Definition>) {
    out.push(
        native::<operation::ClaimTopic>(
            "claim-topic",
            "claimTopic",
            "/topics/{id}/claim",
            "Claim one open Topic for an actor.",
        )
        .target(TargetAction::Claim, Some(NodeType::Topic)),
    );
    out.push(
        native::<operation::ReleaseTopic>(
            "release-topic",
            "releaseTopic",
            "/topics/{id}/release",
            "Release the claim on one Topic.",
        )
        .target(TargetAction::Release, Some(NodeType::Topic)),
    );
    out.push(native::<operation::CloseTopic>(
        "close-topic",
        "closeTopic",
        "/topics/{id}/close",
        "Close one Topic in the bound scope.",
    ));
}

fn question_actions(out: &mut Vec<Definition>) {
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
    out.push(
        native::<operation::AnswerQuestion>(
            "answer-question",
            "answerQuestion",
            "/questions/{id}/answer",
            "Record the answer to one Question.",
        )
        .target(TargetAction::Answer, Some(NodeType::Question)),
    );
}

fn review_actions_for_kind(
    out: &mut Vec<Definition>,
    plural: &'static str,
    singular: &'static str,
    title: &'static str,
    kind: NodeType,
) {
    let base = leaked(format!("/{plural}/{{id}}"));
    out.push(
        backed::<operation::SubmitRequirementReview>(
            leaked(format!("submit-{singular}-review")),
            leaked(format!("submit{title}Review")),
            HttpMethod::Post,
            leaked(format!("{base}/submit")),
            "Submit the current record revision for review.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .scope("scope_id")
        .target(TargetAction::Submit, Some(kind)),
    );
    out.push(
        backed::<operation::DecideRequirementReview>(
            leaked(format!("decide-{singular}-review")),
            leaked(format!("decide{title}Review")),
            HttpMethod::Post,
            leaked(format!("{base}/submissions/{{proposal_id}}/decide")),
            "Decide one record review submission.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("proposal_id")],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .scope("scope_id"),
    );
    out.push(
        backed::<operation::WithdrawRequirementReview>(
            leaked(format!("withdraw-{singular}-review")),
            leaked(format!("withdraw{title}Review")),
            HttpMethod::Post,
            leaked(format!("{base}/submissions/{{proposal_id}}/withdraw")),
            "Withdraw one record review submission.",
            ResponseKind::Resource,
            vec![schema::path("id"), schema::path("proposal_id")],
        )
        .path_field("id", "record_id")
        .fixed("record_kind", kind.as_str())
        .scope("scope_id"),
    );
}

macro_rules! review_actions_for_row {
    ($out:ident, [$kind:ident], [requirements], [$review:ident]) => {
        review_actions_for_kind(
            $out,
            "requirements",
            "requirement",
            "Requirement",
            NodeType::$kind,
        );
    };
    (
        $out:ident,
        [$kind:ident],
        [writable {
            mode: $mode:ident,
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal,
            create: $create:ident,
            update: $update:ident,
            create_defaults: $create_defaults:ident,
            create_aliases: $create_aliases:ident,
            update_defaults: $update_defaults:ident,
            update_aliases: $update_aliases:ident,
            nullable: $nullable:expr,
            target: $target:expr
        }],
        [$review:ident]
    ) => {
        review_actions_for_kind(
            $out,
            $plural,
            $singular,
            $singular_id,
            NodeType::$kind,
        );
    };
    ($out:ident, [$($kind:tt)*], [$($route:tt)*], []) => {};
}

macro_rules! register_review_actions {
    (
        $out:ident;
        $($group:ident {
            $($variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$($node:tt)*],
                reader: { open: $reader:ident, closed: [$($closed:tt)*], strategy: $strategy:ident },
                id: $id:ident,
                loader: [$($loader:tt)*],
                graph: [$($graph:tt)*],
                import: [$($import:tt)*],
                catalog: [$($catalog:tt)*],
                route: [$($route:tt)*]
                $(, review: $review:ident)?
            };)*
        })*
    ) => {
        $($(review_actions_for_row!(
            $out, [$($node)*], [$($route)*], [$($review)?]
        );)*)*
    };
}

fn review_actions(out: &mut Vec<Definition>) {
    crate::cache::family_table::record_family_rows!(register_review_actions, out);
}

fn leaked(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn verification_runs(out: &mut Vec<Definition>) {
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
