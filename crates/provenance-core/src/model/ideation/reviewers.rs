//! The manifest field `disposition_actor_ids`: who can make a review decision
//! and who can withdraw a review submission.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::dispositions::DispositionActor;
use super::IdentityType;

/// The entries of the manifest field `disposition_actor_ids`.
///
/// An actor matches an entry when its identity type and its ID both equal those
/// of the entry; the name has no effect. A matching actor can make a review
/// decision, and the author of a submission or a matching actor can withdraw it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Reviewers(Vec<DispositionActor>);

impl Reviewers {
    /// Lets a matching actor make a review decision.
    pub fn admits_decision(&self, actor: &DispositionActor) -> Result<(), DispositionActorRefusal> {
        if self.matches(actor) {
            Ok(())
        } else if self.0.is_empty() {
            Err(DispositionActorRefusal::NoActorsConfigured(
                actor.identity_type,
            ))
        } else {
            Err(DispositionActorRefusal::ActorNotListed(actor.identity_type))
        }
    }

    /// Lets the author of a review submission or a matching actor withdraw it.
    ///
    /// The submission records only the ID of its author, so the author check
    /// compares the ID. A submission with no author has only the matching actors.
    #[provenance_macros::rule("rule_withdrawal_actor_is_author_or_reviewer")]
    pub fn admits_withdrawal(
        &self,
        actor: &DispositionActor,
        author: Option<&str>,
    ) -> Result<(), DispositionActorRefusal> {
        if author == Some(actor.id.as_str()) || self.matches(actor) {
            Ok(())
        } else if self.0.is_empty() {
            Err(DispositionActorRefusal::WithdrawalNoActorsConfigured(
                actor.identity_type,
            ))
        } else {
            Err(DispositionActorRefusal::WithdrawalActorNotListed(
                actor.identity_type,
            ))
        }
    }

    /// The IDs of the entries with the identity type `human`.
    pub fn people(&self) -> impl Iterator<Item = &str> {
        self.0
            .iter()
            .filter(|entry| entry.identity_type == IdentityType::Human)
            .map(|entry| entry.id.as_str())
    }

    /// The entries with the identity type `agent` or `service`.
    pub fn others(&self) -> impl Iterator<Item = &DispositionActor> {
        self.0
            .iter()
            .filter(|entry| entry.identity_type != IdentityType::Human)
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[provenance_macros::rule("rule_reviewer_entry_matches_identity_type_and_id")]
    fn matches(&self, actor: &DispositionActor) -> bool {
        self.0
            .iter()
            .any(|entry| entry.identity_type == actor.identity_type && entry.id == actor.id)
    }
}

impl FromIterator<DispositionActor> for Reviewers {
    fn from_iter<I: IntoIterator<Item = DispositionActor>>(entries: I) -> Self {
        Self(entries.into_iter().collect())
    }
}

/// The form of one entry, as the refusal of a manifest gives it.
const ENTRY_FORM: &str = "an entry of the manifest field disposition_actor_ids: an object with \
    identity_type and id, for example {\"identity_type\":\"human\",\"id\":\"lead_reviewer\"}";

impl<'de> Deserialize<'de> for Reviewers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries = Vec::<Entry>::deserialize(deserializer)?;
        Ok(entries.into_iter().map(|Entry(actor)| actor).collect())
    }
}

/// One entry of the field, read with a refusal that names the field.
struct Entry(DispositionActor);

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(EntryVisitor)
    }
}

struct EntryVisitor;

impl<'de> Visitor<'de> for EntryVisitor {
    type Value = Entry;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(ENTRY_FORM)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Entry, E> {
        Err(E::custom(format_args!(
            "the manifest field disposition_actor_ids has the text \"{value}\" as an entry. \
             Each entry is an object with identity_type and id, for example \
             {{\"identity_type\":\"human\",\"id\":\"{value}\"}}"
        )))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Entry, A::Error> {
        DispositionActor::deserialize(de::value::MapAccessDeserializer::new(map)).map(Entry)
    }
}

/// The refusal of a review decision or a withdrawal by an actor that no entry
/// of the manifest field matches. Each cause holds only the identity type of
/// the actor, so its text cannot give an actor ID.
#[provenance_macros::rule("rule_decision_refusal_hides_reviewer_ids")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DispositionActorRefusal {
    #[error(
        "The manifest field disposition_actor_ids is empty, thus no actor can make a review \
         decision. {}", next_step(*.0)
    )]
    NoActorsConfigured(IdentityType),
    #[error(
        "The manifest field disposition_actor_ids does not contain the actor of a review \
         decision. {}", next_step(*.0)
    )]
    ActorNotListed(IdentityType),
    #[error(
        "The actor of a withdrawal is not the author of the review submission, and the manifest \
         field disposition_actor_ids is empty. {}", next_step(*.0)
    )]
    WithdrawalNoActorsConfigured(IdentityType),
    #[error(
        "The actor of a withdrawal is not the author of the review submission, and the manifest \
         field disposition_actor_ids does not contain the actor. {}", next_step(*.0)
    )]
    WithdrawalActorNotListed(IdentityType),
}

/// The next step for an actor of `identity_type`. The init flag writes only
/// `human` entries and replaces the list, so the caller gives it for each person.
const fn next_step(identity_type: IdentityType) -> &'static str {
    match identity_type {
        IdentityType::Human => {
            "To change this field, use the command provenance init --disposition-actor-id \
             <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each person that \
             can make a review decision."
        }
        IdentityType::Agent => {
            "The command provenance init writes only entries with the identity type human. To \
             let an actor with the identity type agent make a review decision, add an object \
             with identity_type agent and the ID of the actor to this field in the file \
             .provenance/state/manifest.json."
        }
        IdentityType::Service => {
            "The command provenance init writes only entries with the identity type human. To \
             let an actor with the identity type service make a review decision, add an object \
             with identity_type service and the ID of the actor to this field in the file \
             .provenance/state/manifest.json."
        }
    }
}
