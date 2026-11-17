//! The list check of the manifest field `disposition_actor_ids`.

use crate::{DispositionActor, DispositionActorRefusal, IdentityType, Reviewers};
use provenance_macros::verifies;
use serde_json::json;

fn actor(identity_type: &str, id: &str) -> DispositionActor {
    serde_json::from_value(json!({"identity_type": identity_type, "id": id})).unwrap()
}

fn reviewers(entries: serde_json::Value) -> Reviewers {
    serde_json::from_value(entries).unwrap()
}

#[test]
#[verifies("rule_reviewer_entry_matches_identity_type_and_id", examples)]
fn a_decision_needs_an_entry_with_the_identity_type_and_the_id() {
    let list = reviewers(json!([{"identity_type": "human", "id": "lead", "name": "Lead"}]));
    assert_eq!(list.admits_decision(&actor("human", "lead")), Ok(()));
    let named: DispositionActor =
        serde_json::from_value(json!({"identity_type": "human", "id": "lead", "name": "Other"}))
            .unwrap();
    assert_eq!(list.admits_decision(&named), Ok(()));
    assert_eq!(
        list.admits_decision(&actor("service", "lead")),
        Err(DispositionActorRefusal::ActorNotListed(
            IdentityType::Service
        ))
    );
    assert_eq!(
        list.admits_decision(&actor("human", "other")),
        Err(DispositionActorRefusal::ActorNotListed(IdentityType::Human))
    );
    assert_eq!(
        Reviewers::default().admits_decision(&actor("agent", "lead")),
        Err(DispositionActorRefusal::NoActorsConfigured(
            IdentityType::Agent
        ))
    );
}

#[test]
#[verifies("rule_withdrawal_actor_is_author_or_reviewer", examples)]
fn a_withdrawal_needs_the_author_or_a_matching_entry() {
    let list = reviewers(json!([{"identity_type": "human", "id": "lead"}]));
    let author = Some("shaping_agent");
    for by_author in [
        actor("agent", "shaping_agent"),
        actor("service", "shaping_agent"),
    ] {
        assert_eq!(list.admits_withdrawal(&by_author, author), Ok(()));
    }
    assert_eq!(
        list.admits_withdrawal(&actor("human", "lead"), author),
        Ok(())
    );
    assert_eq!(
        list.admits_withdrawal(&actor("human", "lead"), None),
        Ok(())
    );
    assert_eq!(
        list.admits_withdrawal(&actor("agent", "shaping_agent"), None),
        Err(DispositionActorRefusal::WithdrawalActorNotListed(
            IdentityType::Agent
        ))
    );
    assert_eq!(
        list.admits_withdrawal(&actor("service", "lead"), author),
        Err(DispositionActorRefusal::WithdrawalActorNotListed(
            IdentityType::Service
        ))
    );
    assert_eq!(
        Reviewers::default().admits_withdrawal(&actor("agent", "stranger"), author),
        Err(DispositionActorRefusal::WithdrawalNoActorsConfigured(
            IdentityType::Agent
        ))
    );
}

#[test]
fn the_people_are_the_human_entries() {
    let list = reviewers(json!([
        {"identity_type": "human", "id": "lead"},
        {"identity_type": "agent", "id": "ci_bot"},
        {"identity_type": "service", "id": "lead"}
    ]));
    assert_eq!(list.people().collect::<Vec<_>>(), ["lead"]);
    assert_eq!(
        list.others().cloned().collect::<Vec<_>>(),
        [actor("agent", "ci_bot"), actor("service", "lead")]
    );
}

#[test]
fn a_text_entry_names_the_field_and_the_object_form() {
    let error = serde_json::from_value::<Reviewers>(json!(["lead"])).unwrap_err();
    assert_eq!(
        error.to_string(),
        "the manifest field disposition_actor_ids has the text \"lead\" as an entry. Each entry \
         is an object with identity_type and id, for example \
         {\"identity_type\":\"human\",\"id\":\"lead\"}"
    );
    let error = serde_json::from_value::<Reviewers>(json!([7])).unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid type: integer `7`, expected an entry of the manifest field \
         disposition_actor_ids: an object with identity_type and id, for example \
         {\"identity_type\":\"human\",\"id\":\"lead_reviewer\"}"
    );
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
fn the_refusal_of_a_person_names_the_init_command_and_of_an_agent_the_manifest_file() {
    assert_eq!(
        DispositionActorRefusal::ActorNotListed(IdentityType::Human).to_string(),
        "The manifest field disposition_actor_ids does not contain the actor of a review \
         decision. To change this field, use the command provenance init --disposition-actor-id \
         <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each person that can \
         make a review decision."
    );
    assert_eq!(
        DispositionActorRefusal::WithdrawalActorNotListed(IdentityType::Agent).to_string(),
        "The actor of a withdrawal is not the author of the review submission, and the manifest \
         field disposition_actor_ids does not contain the actor. The command provenance init \
         writes only entries with the identity type human. To let an actor with the identity \
         type agent make a review decision, add an object with identity_type agent and the ID \
         of the actor to this field in the file .provenance/state/manifest.json."
    );
}
