//! Fixed cases that the engine tests and the planted tests share.

use provenance_simulation::generate::{Case, Operation, Step};

/// A section that meets a moved base: the engine enters the section and
/// reads, then a write outside lands before its first publish.
pub fn moved_base_case() -> Case {
    let bind = Operation::BindTwice {
        first: 0,
        second: 1,
    };
    Case {
        engines: 2,
        steps: vec![Step::WriteOnMovedBase {
            engine: 0,
            op: bind,
            turns: 1,
            outside: Operation::PostMessage,
        }],
    }
}

pub const fn write(engine: u8, op: Operation) -> Step {
    Step::Write { engine, op }
}

/// Each write gets enough turns to end before the next step.
pub fn alone(ops: &[Operation]) -> Vec<Step> {
    ops.iter()
        .flat_map(|op| {
            std::iter::once(write(0, op.clone())).chain(std::iter::repeat_n(Step::Turn(0), 200))
        })
        .collect()
}

/// Each operation, in an order where each one has a record to act on.
pub fn each_operation() -> Vec<Operation> {
    vec![
        Operation::CreateRequirement { n: 0 },
        Operation::PostMessage,
        Operation::SubmitReview,
        Operation::WithdrawReview { n: 0 },
        Operation::SubmitReview,
        Operation::DecideReview { n: 1 },
        Operation::WriteDiscussion { reply: false },
        Operation::WriteDiscussion { reply: true },
        Operation::ClaimTopic,
        Operation::ReleaseTopic,
        Operation::UpdateRequirement { n: 1, stale: false },
        Operation::BindTwice {
            first: 0,
            second: 1,
        },
        Operation::ArchiveRequirement { n: 0 },
    ]
}
