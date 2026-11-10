//! The operation sequences that every layer of the simulation runs.
//!
//! Hegel draws a [`Case`]: the number of engines and a list of [`Step`]s.
//! The engine shrinks the choice sequence, so a step needs no shrinker of its
//! own.

use crate::{scope, stable};
use hegel::generators as gs;
use hegel::{DefaultGenerator, TestCase};
use provenance_store::repository::Repository;
use serde::Serialize;

/// The most steps that one case has.
pub const MAX_STEPS: usize = 40;

/// One step of a case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, DefaultGenerator)]
pub enum Step {
    /// Gives `op` to an engine. A busy engine takes a turn instead.
    Write { engine: u8, op: Operation },
    /// Lets one engine make its next storage call.
    Turn(u8),
    /// A fault of the storage or of an engine.
    Fault(Fault),
    /// Changes the state outside every engine, as a merge in git does.
    OutsideWrite(Operation),
}

/// A fault that the simulated storage or an engine meets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, DefaultGenerator)]
pub enum Fault {
    /// The next publish is refused and lands no key.
    Refuse,
    /// The next publish lands `landed` keys and its outcome is unknown.
    UnknownOutcome { landed: u8 },
    /// The engine stops where it is; a new engine takes its place.
    Stop { engine: u8 },
}

/// A write operation of the engine. Small numbers pick a record, so that
/// operations of one case meet on the same records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, DefaultGenerator)]
pub enum Operation {
    CreateRequirement {
        n: u8,
    },
    /// With `stale`, the update gives the etag the record had at the start.
    UpdateRequirement {
        n: u8,
        stale: bool,
    },
    PostMessage,
    SubmitReview,
    WithdrawReview {
        n: u8,
    },
    ClaimTopic,
    ReleaseTopic,
    /// Two verification binding writes in one section.
    BindTwice {
        first: u8,
        second: u8,
    },
}

/// One drawn case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Case {
    pub engines: u8,
    pub steps: Vec<Step>,
}

impl Case {
    /// Draws a case with two or three engines.
    pub fn draw(tc: &TestCase) -> Self {
        let engines = tc.draw(gs::integers::<u8>().min_value(2).max_value(3));
        let steps = tc.draw(gs::vecs(gs::default::<Step>()).max_size(MAX_STEPS));
        Self { engines, steps }
    }
}

/// The scope of every simulated record.
pub const SCOPE: &str = "default";
/// The Requirement in the initial state.
pub const FIRST_REQUIREMENT: &str = "req_expiry";
/// The Rule in the initial state.
pub const FIRST_RULE: &str = "rule_expiry";
/// The Topic in the initial state.
pub const TOPIC: &str = "topic_owner";

/// The Requirement that `n` picks: the first one or one of three new ones.
pub fn requirement_id(n: u8) -> String {
    match n % 4 {
        0 => FIRST_REQUIREMENT.to_owned(),
        other => format!("req_sim_{other}"),
    }
}

/// The Requirement that a create with `n` makes.
pub fn created_requirement_id(n: u8) -> String {
    format!("req_sim_{}", n % 3 + 1)
}

/// The file of a verification binding.
pub fn binding_file(n: u8) -> String {
    format!("tests/sim_{}.test.ts", n % 4)
}

/// The record that an operation writes, for the laws that follow a record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Target {
    /// The description of a Requirement.
    Description(String),
    /// A Requirement that a create makes.
    Exists(String),
    /// The claim of the Topic.
    Claim,
}

/// What an operation that succeeded did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Effect {
    /// The operation had nothing to act on.
    Skipped,
    Created {
        id: String,
    },
    Described {
        id: String,
        description: String,
        etag: String,
    },
    Message {
        id: String,
    },
    Proposal {
        id: String,
    },
    Withdrawn {
        proposal: String,
    },
    Claim {
        holder: Option<String>,
    },
    Bound {
        first: bool,
        second: bool,
    },
}

/// One operation with the values that the case resolved for it.
#[derive(Debug, Clone)]
pub struct Task {
    pub operation: u64,
    /// The engine slot; outside writes have none.
    pub engine: Option<usize>,
    pub op: Operation,
    /// The etag that a stale update gives.
    pub stale_etag: String,
    /// The proposal that a withdrawal names.
    pub proposal: Option<String>,
}

impl Task {
    /// The record that the operation writes.
    pub fn target(&self) -> Option<Target> {
        match &self.op {
            Operation::CreateRequirement { n } => Some(Target::Exists(created_requirement_id(*n))),
            Operation::UpdateRequirement { n, .. } => Some(Target::Description(requirement_id(*n))),
            Operation::ClaimTopic | Operation::ReleaseTopic => Some(Target::Claim),
            _ => None,
        }
    }

    /// The actor of a claim: each engine is its own agent.
    pub fn actor(&self) -> String {
        self.engine
            .map_or_else(|| "outside".to_owned(), |slot| format!("engine-{slot}"))
    }

    /// The description that an update writes.
    pub fn description(&self) -> String {
        format!("Description from operation {}", self.operation)
    }
}

/// What the case knows when it gives an operation out.
#[derive(Debug, Clone)]
pub struct Context {
    first_etag: String,
    proposals: Vec<String>,
}

impl Context {
    pub const fn new(first_etag: String) -> Self {
        Self {
            first_etag,
            proposals: Vec::new(),
        }
    }

    /// Learns the proposals that earlier operations made.
    pub fn learn(&mut self, effects: impl IntoIterator<Item = Effect>) {
        for effect in effects {
            if let Effect::Proposal { id } = effect {
                if !self.proposals.contains(&id) {
                    self.proposals.push(id);
                }
            }
        }
    }

    pub fn task(&self, operation: u64, engine: Option<usize>, op: Operation) -> Task {
        let proposal = match &op {
            Operation::WithdrawReview { n } if !self.proposals.is_empty() => {
                Some(self.proposals[usize::from(*n) % self.proposals.len()].clone())
            }
            _ => None,
        };
        Task {
            operation,
            engine,
            op,
            stale_etag: self.first_etag.clone(),
            proposal,
        }
    }
}

/// Runs one operation on `repository`.
// One arm per operation; splitting them would hide the list.
#[allow(clippy::too_many_lines)]
pub fn apply(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    use provenance_core::{MessageRole, NodeType, RequirementStatus, ThreadParent};
    use provenance_store::review::{SubmitRecordReview, WithdrawRecordReview};
    use provenance_store::state_store::{
        CreateRequirementInput, MaterializeVerificationBindingInput, PostMessageInput,
        UpdateRequirementInput,
    };
    let store = repository.store();
    let first = || stable(FIRST_REQUIREMENT);
    Ok(match &task.op {
        Operation::CreateRequirement { n } => {
            let id = created_requirement_id(*n);
            store.create_requirement(CreateRequirementInput {
                scope_id: scope(),
                id: stable(&id),
                statement: "Share links have an owner".into(),
                description: None,
                status: RequirementStatus::Active,
                refines: Vec::new(),
                depends_on: Vec::new(),
                supersedes: Vec::new(),
                spawned_by: None,
                origin_thread: None,
                origin_message: None,
            })?;
            Effect::Created { id }
        }
        Operation::UpdateRequirement { n, stale } => {
            let id = requirement_id(*n);
            let etag = if *stale {
                task.stale_etag.clone()
            } else {
                store.requirement_edit_state(&scope(), &stable(&id))?.etag
            };
            let description = task.description();
            store.update_requirement(UpdateRequirementInput {
                scope_id: scope(),
                id: stable(&id),
                expected_etag: Some(etag.clone()),
                declared_by: None,
                statement: None,
                description: Some(description.clone()),
                status: None,
                archived_in_commit: None,
                clear_fields: Vec::new(),
            })?;
            Effect::Described {
                id,
                description,
                etag,
            }
        }
        Operation::PostMessage => {
            let posted = store.post_thread_message(PostMessageInput {
                scope_id: scope(),
                parent: ThreadParent {
                    node_type: NodeType::Requirement,
                    node_id: first(),
                },
                role: MessageRole::User,
                body: format!("Message from operation {}", task.operation),
            })?;
            Effect::Message {
                id: posted.message.id.as_str().to_owned(),
            }
        }
        Operation::SubmitReview => {
            let entry = store.submit_record_review(SubmitRecordReview {
                scope_id: scope(),
                actor: "reviewer".into(),
                record_kind: NodeType::Requirement,
                record_id: first(),
                declared_by: None,
                title: "Simulated review".into(),
                summary: format!("Review from operation {}", task.operation),
                confidence: None,
                source_ids: Vec::new(),
                evidence_references: Vec::new(),
                builds_on: Vec::new(),
                expected_revision: None,
                revises: None,
            })?;
            Effect::Proposal {
                id: entry.proposal_id.as_str().to_owned(),
            }
        }
        Operation::WithdrawReview { .. } => {
            let Some(proposal) = &task.proposal else {
                return Ok(Effect::Skipped);
            };
            store.withdraw_record_review(WithdrawRecordReview {
                scope_id: scope(),
                actor: "reviewer".into(),
                proposal_id: stable(proposal),
                declared_by: None,
                reason: None,
            })?;
            Effect::Withdrawn {
                proposal: proposal.clone(),
            }
        }
        Operation::ClaimTopic => {
            store.claim_topic(&scope(), &stable(TOPIC), &task.actor())?;
            Effect::Claim {
                holder: Some(task.actor()),
            }
        }
        Operation::ReleaseTopic => {
            store.release_topic(&scope(), &stable(TOPIC), &task.actor())?;
            Effect::Claim { holder: None }
        }
        Operation::BindTwice { first, second } => {
            let binding = |n: u8| MaterializeVerificationBindingInput {
                scope_id: scope(),
                rule_id: stable(FIRST_RULE),
                key: "simulated-binding".into(),
                method: provenance_core::VerificationMethod::Examples,
                declared_by: "test://simulation".into(),
                file: binding_file(n).into(),
                symbol: None,
            };
            let (first, second) = store.with_repository_publication(|section| {
                let first = section.materialize_verification_binding(binding(*first));
                let second = section.materialize_verification_binding(binding(*second));
                Ok((first.is_ok(), second.is_ok()))
            })?;
            Effect::Bound { first, second }
        }
    })
}
