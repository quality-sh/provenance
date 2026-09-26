//! Complete collection reads used by the resource routes.
use super::scoped_list::scoped_list;

scoped_list!(ListSourcesV2, "list-sources", Source, list_sources);
scoped_list!(
    ListRequirementsV2,
    "list-requirements",
    Requirement,
    list_requirements
);
scoped_list!(
    ListResolutionsV2,
    "list-resolutions",
    Resolution,
    list_resolutions
);
scoped_list!(ListRulesV2, "list-rules", Rule, list_rules);
scoped_list!(ListDomainsV2, "list-domains", Domain, list_domains);
scoped_list!(
    ListBoundariesV2,
    "list-boundaries",
    Boundary,
    list_boundaries
);
scoped_list!(ListTopicsV2, "list-topics", Topic, list_topics);
scoped_list!(ListQuestionsV2, "list-questions", Question, list_questions);
scoped_list!(
    ListContributionsV2,
    "list-contributions",
    Contribution,
    list_contributions
);
scoped_list!(
    ListSynthesisPacketsV2,
    "list-synthesis-packets",
    SynthesisPacket,
    list_synthesis_packets
);
scoped_list!(
    ListProposalsV2,
    "list-proposals-v2",
    ProposalCard,
    list_proposal_cards
);
scoped_list!(
    ListVerificationRunsV2,
    "list-verification-runs",
    VerificationRun,
    list_verification_runs
);
scoped_list!(
    ListVerificationBindingsV2,
    "list-verification-bindings",
    VerificationBinding,
    list_verification_bindings
);
scoped_list!(
    ListDiscussionContainersV2,
    "list-discussion-containers",
    Thread,
    list_threads
);
scoped_list!(ListMessagesV2, "list-messages-v2", Message, list_messages);
scoped_list!(
    ListAssertionsV2,
    "list-assertions-v2",
    AssertionRecord,
    list_assertion_records
);
scoped_list!(
    ListDispositionsV2,
    "list-dispositions-v2",
    DispositionRecord,
    list_dispositions
);
