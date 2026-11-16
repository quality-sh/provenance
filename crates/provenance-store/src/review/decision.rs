//! The guarded decision cycle for every enrolled graph record kind.

mod resolution;
mod submission;
mod withdrawal;

use provenance_core::{IdeationTargetType, NodeType};

const fn target_type(kind: NodeType) -> IdeationTargetType {
    match kind {
        NodeType::Source => IdeationTargetType::Source,
        NodeType::Requirement => IdeationTargetType::Requirement,
        NodeType::Resolution => IdeationTargetType::Resolution,
        NodeType::Rule => IdeationTargetType::Rule,
        NodeType::Topic => IdeationTargetType::Topic,
        NodeType::Question => IdeationTargetType::Question,
    }
}
