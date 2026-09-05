use camino::Utf8PathBuf;
use provenance_core::ScopeId;

use crate::layout::ProvenanceLayout;

pub fn sources_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("sources/source.jsonl")
}

pub fn requirements_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("requirements/req.jsonl")
}

pub fn domains_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("domains/domain.jsonl")
}

pub fn boundaries_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("boundaries/boundary.jsonl")
}

pub fn topics_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("topics/topic.jsonl")
}

pub fn questions_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("questions/question.jsonl")
}

pub fn resolutions_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("resolutions/res.jsonl")
}

pub fn rules_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("rules/rule.jsonl")
}

pub fn verification_bindings_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("verifications/binding.jsonl")
}

pub fn requirement_reviews_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("requirements/review.jsonl")
}

pub fn implementation_bindings_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("implementations/binding.jsonl")
}

pub fn threads_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("threads/threads.jsonl")
}

pub fn messages_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("threads/2026-07.jsonl")
}

pub fn contributions_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/contributions.jsonl")
}

pub fn synthesis_packets_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/synthesis_packets.jsonl")
}

pub fn proposal_cards_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/proposal_cards.jsonl")
}

pub fn dispositions_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/dispositions.jsonl")
}

pub(crate) fn legacy_promotion_decisions_path(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/promotion_decisions.jsonl")
}

pub fn assertion_records_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/assertions.jsonl")
}

pub fn ideation_landings_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/landings.jsonl")
}
