//! Who can withdraw a review submission, through the CLI api command, the HTTP
//! host and MCP. The author of the submission or an actor that matches an
//! entry of the manifest field `disposition_actor_ids` by identity type and ID
//! can withdraw it; each other actor gets `access_denied`. A submission from a
//! write that gives no actor has no author.

use crate::cli_review_decision_identity::{actor, Review, AGENT_NEXT_STEP, LEAD, PERSON_NEXT_STEP};
use crate::review_host_support::surfaces::SURFACES;

use provenance_macros::verifies;
use serde_json::{json, Value};

/// The actor that writes the submission.
const AUTHOR: &str = "shaping_agent";
/// An actor that is not the author and is not in the field.
const STRANGER: &str = "stranger";
/// The cause of the refusal when the field holds other actors.
const NOT_LISTED: &str = "The actor of a withdrawal is not the author of the review submission, \
    and the manifest field disposition_actor_ids does not contain the actor.";
/// The cause of the refusal when the field is empty.
const EMPTY: &str = "The actor of a withdrawal is not the author of the review submission, and \
    the manifest field disposition_actor_ids is empty.";
fn withdrawal(actor: &Value) -> Value {
    json!({"actor": actor, "declared_by": null, "reason": null})
}

fn access_denied(cause: &str) -> Value {
    json!({"kind": "access_denied", "message": format!("{cause} {AGENT_NEXT_STEP}")})
}

/// The stored Withdrawal rows of the repository of `review`.
fn stored_withdrawals(review: &Review) -> Vec<Value> {
    let path = review
        .root()
        .join(".provenance/state/scopes/default/ideation/withdrawals.jsonl");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Checks that `withdrawer` withdraws the submission of a new review with the
/// field `actor_ids` and the author `author`, and that the stored Withdrawal
/// records `withdrawer`.
fn assert_withdraws(actor_ids: &[&str], author: Option<&str>, withdrawer: &Value) {
    for surface in SURFACES {
        let review = Review::new(actor_ids, author);
        let answer = review.post(surface, "withdraw", &withdrawal(withdrawer));
        assert_eq!(answer.code, surface.landed(), "{surface:?} {}", answer.text);
        assert_eq!(answer.body["fact"], "withdrawn", "{surface:?}");
        assert_eq!(answer.body["actor"], withdrawer["id"], "{surface:?}");
        assert_eq!(review.decision()["withdrawn"], json!([review.proposal]));
        let stored = stored_withdrawals(&review);
        assert_eq!(stored.len(), 1, "{surface:?}");
        assert_eq!(&stored[0]["actor"], withdrawer, "{surface:?}");
    }
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn the_author_withdraws_and_the_withdrawal_records_the_actor() {
    assert_withdraws(&[LEAD], Some(AUTHOR), &actor("agent", AUTHOR));
}

/// The submission records only the ID of its author, so the author check
/// compares the ID: a service with the ID of the author withdraws.
#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn the_author_check_compares_the_id_only() {
    assert_withdraws(&[LEAD], Some(AUTHOR), &actor("service", AUTHOR));
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn the_listed_person_withdraws() {
    assert_withdraws(&[LEAD], Some(AUTHOR), &actor("human", LEAD));
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn the_listed_person_withdraws_a_submission_with_no_author() {
    assert_withdraws(&[LEAD], None, &actor("human", LEAD));
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
#[verifies("rule_withdrawal_refusal_is_access_denied", examples)]
fn the_placeholder_authoring_is_not_an_author() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], None);
        review.assert_refused(surface, 403, &access_denied(NOT_LISTED), || {
            review.post(
                surface,
                "withdraw",
                &withdrawal(&actor("agent", "authoring")),
            )
        });
    }
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
#[verifies("rule_withdrawal_refusal_is_access_denied", examples)]
fn a_third_actor_cannot_withdraw() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], Some(AUTHOR));
        review.assert_refused(surface, 403, &access_denied(NOT_LISTED), || {
            review.post(surface, "withdraw", &withdrawal(&actor("agent", STRANGER)))
        });
    }
}

#[test]
#[verifies("rule_withdrawal_refusal_is_access_denied", examples)]
fn a_third_actor_cannot_withdraw_when_the_field_is_empty() {
    for surface in SURFACES {
        let review = Review::new(&[], Some(AUTHOR));
        review.assert_refused(surface, 403, &access_denied(EMPTY), || {
            review.post(surface, "withdraw", &withdrawal(&actor("agent", STRANGER)))
        });
    }
}

#[test]
#[verifies("rule_withdrawal_refusal_is_access_denied", examples)]
fn a_person_that_is_not_listed_gets_the_init_command() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], Some(AUTHOR));
        let message = format!("{NOT_LISTED} {PERSON_NEXT_STEP}");
        let expected = json!({"kind": "access_denied", "message": message});
        review.assert_refused(surface, 403, &expected, || {
            review.post(surface, "withdraw", &withdrawal(&actor("human", STRANGER)))
        });
    }
}

#[test]
fn an_actor_as_text_is_invalid_input() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], Some(AUTHOR));
        let expected =
            json!({"kind": "invalid_input", "field": "request.actor", "reason": "invalid_value"});
        review.assert_refused(surface, 400, &expected, || {
            review.post(surface, "withdraw", &withdrawal(&json!(LEAD)))
        });
    }
}

/// The state checks of the submission come before the actor check.
#[test]
fn a_third_actor_after_a_withdrawal_gets_the_submission_conflict() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], Some(AUTHOR));
        let first = review.post(surface, "withdraw", &withdrawal(&actor("agent", AUTHOR)));
        assert_eq!(first.code, surface.landed(), "{surface:?} {}", first.text);
        let revision = review.decision()["current_revision"].clone();
        let expected = json!({
            "kind": "review_submission_conflict", "current_revision": revision,
            "current_submission": null
        });
        review.assert_refused(surface, 409, &expected, || {
            review.post(surface, "withdraw", &withdrawal(&actor("agent", STRANGER)))
        });
    }
}

/// The existence check of the submission comes before the actor check.
#[test]
fn a_third_actor_on_an_unknown_submission_gets_not_found() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], Some(AUTHOR));
        let unknown = "00000000-0000-4000-8000-000000000000";
        let expected = json!({"kind": "resource_not_found"});
        review.assert_refused(surface, 404, &expected, || {
            review.post_to(
                surface,
                unknown,
                "withdraw",
                &withdrawal(&actor("agent", STRANGER)),
            )
        });
    }
}

/// The owner check of the record comes before the actor check.
#[test]
#[verifies("rule_ownership_refusal_names_owner", examples)]
fn a_third_actor_with_a_wrong_owner_gets_the_ownership_conflict() {
    for surface in SURFACES {
        let review = Review::new(&[LEAD], Some(AUTHOR));
        let mut input = withdrawal(&actor("agent", STRANGER));
        input["declared_by"] = json!("another_agent");
        let expected = json!({
            "kind": "record_ownership_conflict", "owner": null,
            "message": "This is a manual record; declared-by claims ownership. CLI: omit \
                --declared-by; MCP: omit declared_by."
        });
        review.assert_refused(surface, 409, &expected, || {
            review.post(surface, "withdraw", &input)
        });
    }
}
