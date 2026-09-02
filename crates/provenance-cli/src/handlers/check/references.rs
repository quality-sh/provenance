use super::index::CheckIndex;
use provenance_core::{
    ArtifactLink, ArtifactLinkTargetType, IdeationTarget, IdeationTargetType, NodeType, ScopeId,
    StableId,
};

pub(super) fn check_origin_references(
    index: &CheckIndex,
    dangling: &mut Vec<String>,
    scope_id: &ScopeId,
    owner: &str,
    origin_thread: Option<&StableId>,
    origin_message: Option<&StableId>,
) {
    if let Some(origin_thread) = origin_thread {
        check_scoped_reference(
            index,
            dangling,
            scope_id,
            owner,
            "origin_thread",
            "thread",
            origin_thread,
        );
    }
    if let Some(origin_message) = origin_message {
        check_scoped_reference(
            index,
            dangling,
            scope_id,
            owner,
            "origin_message",
            "message",
            origin_message,
        );
    }
}

pub(super) fn check_artifact_links(
    index: &CheckIndex,
    dangling: &mut Vec<String>,
    scope_id: &ScopeId,
    owner: &str,
    links: &[ArtifactLink],
) {
    for link in links {
        check_scoped_reference(
            index,
            dangling,
            scope_id,
            owner,
            "link",
            artifact_link_target_name(link.target_type),
            &link.target_id,
        );
    }
}

pub(super) fn check_ideation_target(
    index: &CheckIndex,
    dangling: &mut Vec<String>,
    scope_id: &ScopeId,
    owner: &str,
    target: &IdeationTarget,
) {
    check_scoped_reference(
        index,
        dangling,
        scope_id,
        owner,
        "target",
        ideation_target_type_name(target.artifact_type),
        &target.artifact_id,
    );
}

pub(super) fn check_scoped_reference(
    index: &CheckIndex,
    dangling: &mut Vec<String>,
    scope_id: &ScopeId,
    owner: &str,
    relation: &str,
    target_type: &str,
    target_id: &StableId,
) {
    if !index.has_scoped_node(scope_id, target_type, target_id) {
        dangling.push(format!(
            "{owner} has dangling reference: {relation} {target_type} {}",
            target_id.as_str()
        ));
    }
}

pub(super) const fn node_type_name(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "resolution",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
        NodeType::Domain => "domain",
        NodeType::Boundary => "boundary",
    }
}

const fn artifact_link_target_name(target_type: ArtifactLinkTargetType) -> &'static str {
    match target_type {
        ArtifactLinkTargetType::Source => "source",
        ArtifactLinkTargetType::Requirement => "requirement",
        ArtifactLinkTargetType::Resolution => "resolution",
        ArtifactLinkTargetType::Rule => "rule",
    }
}

const fn ideation_target_type_name(target_type: IdeationTargetType) -> &'static str {
    match target_type {
        IdeationTargetType::Source => "source",
        IdeationTargetType::Requirement => "requirement",
        IdeationTargetType::Resolution => "resolution",
        IdeationTargetType::Rule => "rule",
        IdeationTargetType::Topic => "topic",
        IdeationTargetType::Question => "question",
        IdeationTargetType::Domain => "domain",
    }
}
