//! The operation sequences that every layer of the simulation runs.
//!
//! Hegel draws a [`Case`]: the number of engines and a list of [`Step`]s.
//! The engine shrinks the choice sequence, so a step needs no shrinker of its
//! own. [`apply`] runs an operation on the store; [`Call`] is the same
//! operation as a call of the public contract.

mod apply;
mod call;

pub use apply::apply;
pub use call::Call;

use hegel::generators as gs;
use hegel::{DefaultGenerator, TestCase};
use serde::Serialize;

/// The most steps that one case has.
pub const MAX_STEPS: usize = 40;

/// One step of a case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, DefaultGenerator)]
pub enum Step {
    /// Gives `op` to an engine. A busy engine takes a turn instead.
    Write { engine: u8, op: Operation },
    /// Gives `op` to an engine, gives the engine `turns` turns, then lands
    /// `outside` outside every engine. So the engine meets a moved base when
    /// it reads in its section before the outside write and publishes after
    /// it.
    WriteOnMovedBase {
        engine: u8,
        op: Operation,
        turns: u8,
        outside: Operation,
    },
    /// Lets one engine make its next storage call.
    Turn(u8),
    /// A fault of the storage or of an engine.
    Fault(Fault),
    /// Changes the state outside every engine, as a merge in git does.
    OutsideWrite(Operation),
}

impl Step {
    /// The steps that this step stands for: a write on a moved base is a
    /// write, its turns and an outside write.
    pub fn expand(&self) -> Vec<Self> {
        match self {
            Self::WriteOnMovedBase {
                engine,
                op,
                turns,
                outside,
            } => {
                let mut steps = vec![Self::Write {
                    engine: *engine,
                    op: op.clone(),
                }];
                steps.extend(std::iter::repeat_n(Self::Turn(*engine), 1 + usize::from(*turns % 3)));
                steps.push(Self::OutsideWrite(outside.clone()));
                steps
            }
            other => vec![other.clone()],
        }
    }
}

/// A fault that the simulated storage or an engine meets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, DefaultGenerator)]
pub enum Fault {
    /// The next publish is refused and lands no key.
    Refuse,
    /// The next publish lands `landed` keys and its outcome is unknown.
    UnknownOutcome { landed: u8 },
    /// The engine stops where it is; a new engine takes its place. The CLI
    /// layer kills the write process at `at` / 255 of the measured time of
    /// a write.
    Stop { engine: u8, at: u8 },
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
    /// Rejects a review submission that an earlier operation made. The
    /// decision lands in a stage.
    DecideReview {
        n: u8,
    },
    /// Starts a Discussion on the first Requirement, or replies to one that
    /// an earlier operation started, on the version that it last knew.
    WriteDiscussion {
        reply: bool,
    },
    /// Archives a Requirement. The archive of its dependents lands in a
    /// stage.
    ArchiveRequirement {
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

impl Operation {
    /// Tells if the operation always lands a stage when it succeeds. Other
    /// review writes stage too, and an archive stages when it archives a
    /// Requirement that was not archived.
    pub const fn always_stages(&self) -> bool {
        matches!(self, Self::DecideReview { .. })
    }
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

    /// The steps of the case with each step expanded.
    pub fn expanded(&self) -> Vec<Step> {
        self.steps.iter().flat_map(Step::expand).collect()
    }
}

/// The scope of every simulated record.
pub const SCOPE: &str = "default";
/// The Requirement in the initial state.
pub const FIRST_REQUIREMENT: &str = "req_expiry";
/// The Requirement in the initial state that refines the first one, so an
/// archive of the first one archives it too.
pub const DETAIL_REQUIREMENT: &str = "req_expiry_detail";
/// The Rule in the initial state.
pub const FIRST_RULE: &str = "rule_expiry";
/// The Topic in the initial state.
pub const TOPIC: &str = "topic_owner";
/// The actor that the manifest lets decide a review.
pub const DECIDER: &str = "reviewer-lead";
/// The commit that an archive names.
pub const ARCHIVE_COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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
    Decided {
        proposal: String,
    },
    Discussion {
        id: String,
        version: u64,
    },
    Archived {
        id: String,
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
    /// The proposal that a withdrawal or a decision names.
    pub proposal: Option<String>,
    /// The Discussion and the version that a reply names.
    pub discussion: Option<(String, u64)>,
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

    /// The body of a message or a Discussion entry.
    pub fn body(&self) -> String {
        format!("Message from operation {}", self.operation)
    }
}

/// What the case knows when it gives an operation out.
#[derive(Debug, Clone)]
pub struct Context {
    first_etag: String,
    proposals: Vec<String>,
    discussions: Vec<(String, u64)>,
}

impl Context {
    pub const fn new(first_etag: String) -> Self {
        Self {
            first_etag,
            proposals: Vec::new(),
            discussions: Vec::new(),
        }
    }

    /// Learns the proposals and the Discussions that earlier operations made.
    pub fn learn(&mut self, effects: impl IntoIterator<Item = Effect>) {
        for effect in effects {
            match effect {
                Effect::Proposal { id } if !self.proposals.contains(&id) => {
                    self.proposals.push(id);
                }
                Effect::Discussion { id, version } => {
                    match self.discussions.iter_mut().find(|(known, _)| *known == id) {
                        Some(known) => known.1 = known.1.max(version),
                        None => self.discussions.push((id, version)),
                    }
                }
                _ => {}
            }
        }
    }

    pub fn task(&self, operation: u64, engine: Option<usize>, op: Operation) -> Task {
        let pick = |n: u8, count: usize| usize::from(n) % count;
        let proposal = match &op {
            Operation::WithdrawReview { n } | Operation::DecideReview { n }
                if !self.proposals.is_empty() =>
            {
                Some(self.proposals[pick(*n, self.proposals.len())].clone())
            }
            _ => None,
        };
        let discussion = match &op {
            Operation::WriteDiscussion { reply: true } => self.discussions.last().cloned(),
            _ => None,
        };
        Task {
            operation,
            engine,
            op,
            stale_etag: self.first_etag.clone(),
            proposal,
            discussion,
        }
    }
}
