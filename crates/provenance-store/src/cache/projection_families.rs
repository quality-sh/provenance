//! The one table of canonical record families.
//!
//! Each entry binds a Rust type to its stable storage and read behavior.
//! Consumers expand this table to build readers, exports, cache loaders, and
//! catalog operations. A new canonical record kind therefore starts here.

use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use camino::Utf8PathBuf;
use provenance_core::ScopeId;

macro_rules! record_families {
    ($consumer:ident) => {
        $consumer! {
            export {
                Sources: provenance_core::Source, sources, sources_path, "sources/source.jsonl", "sources", [Source], list_sources, [closed_sources], field, [record(Source)], [projection(ListSourcesV2, "list-sources", PageSourcesV2, "page-sources-v2", GetSourceV2, "get-source-v2")];
                Domains: provenance_core::Domain, domains, domains_path, "domains/domain.jsonl", "domains", [Domain], list_domains, [closed_domains], field, [record(Domain)], [projection(ListDomainsV2, "list-domains", PageDomainsV2, "page-domains-v2", GetDomainV2, "get-domain-v2")];
                Requirements: provenance_core::Requirement, requirements, requirements_path, "requirements/req.jsonl", "requirements", [Requirement], list_requirements, [closed_requirements], field, [record(Requirement)], [projection(ListRequirementsV2, "list-requirements", PageRequirementsV2, "page-requirements-v2", none)];
                Boundaries: provenance_core::Boundary, boundaries, boundaries_path, "boundaries/boundary.jsonl", "boundaries", [Boundary], list_boundaries, [closed_boundaries], field, [record(Boundary)], [projection(ListBoundariesV2, "list-boundaries", PageBoundariesV2, "page-boundaries-v2", GetBoundaryV2, "get-boundary-v2")];
                Topics: provenance_core::Topic, topics, topics_path, "topics/topic.jsonl", "topics", [Topic], list_topics, [closed_topics], field, [record(Topic)], [projection(ListTopicsV2, "list-topics", PageTopicsV2, "page-topics-v2", GetTopicV2, "get-topic-v2")];
                Questions: provenance_core::Question, questions, questions_path, "questions/question.jsonl", "questions", [Question], list_questions, [closed_questions], field, [record(Question)], [projection(ListQuestionsV2, "list-questions", PageQuestionsV2, "page-questions-v2", GetQuestionV2, "get-question-v2")];
                Resolutions: provenance_core::Resolution, resolutions, resolutions_path, "resolutions/res.jsonl", "resolutions", [Resolution], list_resolutions, [closed_resolutions], field, [record(Resolution)], [projection(ListResolutionsV2, "list-resolutions", PageResolutionsV2, "page-resolutions-v2", GetResolutionV2, "get-resolution-v2")];
                Rules: provenance_core::Rule, rules, rules_path, "rules/rule.jsonl", "rules", [Rule], list_rules, [closed_rules], field, [record(Rule)], [projection(ListRulesV2, "list-rules", PageRulesV2, "page-rules-v2", GetRuleV2, "get-rule-v2")];
                ImplementationBindings: provenance_core::ImplementationBinding, implementation_bindings, implementation_bindings_path, "implementations/binding.jsonl", "implementation_bindings", [], list_implementation_bindings, [closed_implementation_bindings], field, [kind], [none];
                VerificationBindings: provenance_core::VerificationBinding, verification_bindings, verification_bindings_path, "verifications/binding.jsonl", "verification_bindings", [], list_verification_bindings, [closed_verification_bindings], field, [kind], [verification(ListVerificationBindingsV2, "list-verification-bindings", PageVerificationBindingsV2, GetVerificationBindingV2, "get-verification-binding-v2")];
            }
            canonical {
                Threads: provenance_core::Thread, threads, threads_path, "threads/threads.jsonl", "threads", [], list_threads, [], field, [payload(load_threads)], [payload(ListDiscussionContainersV2, "list-discussion-containers", PageDiscussionContainersV2, "page-discussion-containers-v2", GetDiscussionContainerV2, "get-discussion-container-v2")];
                Messages: provenance_core::Message, messages, messages_path, "threads/2026-07.jsonl", "messages", [], list_messages, [], field, [payload(load_messages)], [payload(ListMessagesV2, "list-messages-v2", PageMessagesV2, "page-messages-v2", GetMessageV2, "get-message-v2")];
                Contributions: provenance_core::Contribution, contributions, contributions_path, "ideation/contributions.jsonl", "contributions", [], list_contributions, [], field, [payload(load_contributions)], [payload(ListContributionsV2, "list-contributions", PageContributionsV2, "page-contributions-v2", GetContributionV2, "get-contribution-v2")];
                SynthesisPackets: provenance_core::SynthesisPacket, synthesis_packets, synthesis_packets_path, "ideation/synthesis_packets.jsonl", "synthesis_packets", [], list_synthesis_packets, [], field, [payload(load_synthesis_packets)], [payload(ListSynthesisPacketsV2, "list-synthesis-packets", PageSynthesisPacketsV2, "page-synthesis-packets-v2", GetSynthesisPacketV2, "get-synthesis-packet-v2")];
                ProposalCards: provenance_core::ProposalCard, proposal_cards, proposal_cards_path, "ideation/proposal_cards.jsonl", "proposal_cards", [], list_proposal_cards, [], field, [payload(load_proposal_cards)], [payload(ListProposalsV2, "list-proposals-v2", PageProposalsV2, "page-proposals-v2", GetProposalV2, "get-proposal-v2")];
                AssertionRecords: provenance_core::AssertionRecord, assertion_records, assertion_records_path, "ideation/assertions.jsonl", "assertion_records", [], list_assertion_records, [], field, [payload(load_assertion_records)], [payload(ListAssertionsV2, "list-assertions-v2", PageAssertionsV2, "page-assertions-v2", GetAssertionV2, "get-assertion-v2")];
                Dispositions: provenance_core::DispositionRecord, dispositions, dispositions_path, "ideation/dispositions.jsonl", "dispositions", [], list_dispositions, [], field, [payload(load_dispositions)], [payload(ListDispositionsV2, "list-dispositions-v2", PageDispositionsV2, "page-dispositions-v2", GetDispositionV2, "get-disposition-v2")];
            }
            internal {
                RequirementReviews: provenance_core::RequirementReview, requirement_reviews, requirement_reviews_path, "requirements/review.jsonl", "requirement_reviews", [], list_requirement_reviews, [], field, [kind], [none];
                ReviewJournal: provenance_core::review::JournalEntry, review_journal, review_journal_path, "review/journal", "review_journal", [], validated_journal_entries, [], method, [journal], [none];
            }
        }
    };
}

pub(crate) use record_families;

macro_rules! define_projection_families {
    (
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        internal { $($internal_variant:ident: $internal_type:ty, $internal_field:ident, $internal_path:ident, $internal_suffix:literal, $internal_table:literal, [$($internal_node:tt)*], $internal_reader:ident, [$($internal_closed:tt)*], $internal_id:ident, [$($internal_loader:tt)*], [$($internal_catalog:tt)*];)* }
    ) => {
        /// One family of canonical records stored in the projection.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProjectionFamily {
            $($export_variant,)*
            $($canonical_variant,)*
            $($internal_variant,)*
        }

        impl ProjectionFamily {
            pub const ALL: [Self; 19] = [
                $(Self::$export_variant,)*
                $(Self::$canonical_variant,)*
                $(Self::$internal_variant,)*
            ];

            pub const fn family_name(self) -> &'static str {
                match self {
                    $(Self::$export_variant => $export_table,)*
                    $(Self::$canonical_variant => $canonical_table,)*
                    $(Self::$internal_variant => $internal_table,)*
                }
            }

            pub const fn shard_suffix(self) -> &'static str {
                match self {
                    $(Self::$export_variant => $export_suffix,)*
                    $(Self::$canonical_variant => $canonical_suffix,)*
                    $(Self::$internal_variant => $internal_suffix,)*
                }
            }

            pub const fn graph_field(self) -> Option<&'static str> {
                match self {
                    $(Self::$export_variant => Some(stringify!($export_field)),)*
                    _ => None,
                }
            }

            pub(crate) const fn node_type(self) -> Option<provenance_core::NodeType> {
                match self {
                    $(Self::$export_variant => family_node_type!($($export_node)*),)*
                    $(Self::$canonical_variant => family_node_type!($($canonical_node)*),)*
                    $(Self::$internal_variant => family_node_type!($($internal_node)*),)*
                }
            }

            pub(crate) fn shard_path(
                self,
                layout: &ProvenanceLayout,
                scope: &ScopeId,
            ) -> Utf8PathBuf {
                layout.scopes_dir().join(scope.as_str()).join(self.shard_suffix())
            }

            pub(crate) fn canonical_records(
                self,
                store: &StateStore,
                scope: &ScopeId,
            ) -> anyhow::Result<(Vec<u8>, u64)> {
                match self {
                    $(Self::$export_variant => sorted_bytes(store.$export_reader(scope)?, family_id!($export_id)),)*
                    $(Self::$canonical_variant => sorted_bytes(store.$canonical_reader(scope)?, family_id!($canonical_id)),)*
                    $(Self::$internal_variant => sorted_bytes(store.$internal_reader(scope)?, family_id!($internal_id)),)*
                }
            }
        }
    };
}

macro_rules! family_node_type {
    () => {
        None
    };
    ($node:ident) => {
        Some(provenance_core::NodeType::$node)
    };
}

macro_rules! family_id {
    (field) => {
        |record| record.id.as_str()
    };
    (method) => {
        |record| record.id().as_str()
    };
}

record_families!(define_projection_families);

impl ProjectionFamily {
    pub(crate) fn content_digest(self, bytes: &[u8]) -> anyhow::Result<String> {
        if !matches!(
            self,
            Self::Sources | Self::Requirements | Self::Rules | Self::Resolutions
        ) {
            return Ok(crate::canonical_digest::digest(bytes));
        }
        let mut records: Vec<serde_json::Value> = serde_json::from_slice(bytes)?;
        for record in &mut records {
            if let Some(record) = record.as_object_mut() {
                record.remove("created");
                record.remove("updated");
            }
        }
        Ok(crate::canonical_digest::digest(
            &crate::canonical_digest::canonical_bytes(&records)?,
        ))
    }
}

fn sorted_bytes<T: serde::Serialize>(
    mut records: Vec<T>,
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<(Vec<u8>, u64)> {
    records.sort_by(|left, right| id(left).cmp(id(right)));
    let count = records.len() as u64;
    Ok((crate::canonical_digest::canonical_bytes(&records)?, count))
}

#[cfg(test)]
mod tests {
    use super::ProjectionFamily;

    #[test]
    fn family_metadata_keeps_stable_storage_and_export_names_together() {
        assert_eq!(ProjectionFamily::Sources.family_name(), "sources");
        assert_eq!(
            ProjectionFamily::Sources.shard_suffix(),
            "sources/source.jsonl"
        );
        assert_eq!(ProjectionFamily::Sources.graph_field(), Some("sources"));
        assert_eq!(
            ProjectionFamily::SynthesisPackets.family_name(),
            "synthesis_packets"
        );
        assert_eq!(
            ProjectionFamily::SynthesisPackets.shard_suffix(),
            "ideation/synthesis_packets.jsonl"
        );
        assert_eq!(ProjectionFamily::SynthesisPackets.graph_field(), None);
    }
}
