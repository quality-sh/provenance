//! Canonical record-family metadata.
//!
//! A new canonical record kind must have one entry in this table. Each entry
//! names its storage, readers, projection, import checks, operations, and route.

macro_rules! record_family_rows {
    ($consumer:path) => {
        $crate::cache::family_table::record_family_rows!(@expand ($consumer) []);
    };
    ($consumer:path, $argument:ident) => {
        $crate::cache::family_table::record_family_rows!(@expand ($consumer) [$argument;]);
    };
    ($consumer:path, $first:ident, $second:ident) => {
        $crate::cache::family_table::record_family_rows!(@expand ($consumer) [$first; $second;]);
    };
    ($consumer:path, $first:ident, $second:ident, $third:ident) => {
        $crate::cache::family_table::record_family_rows!(
            @expand ($consumer) [$first; $second; $third;]
        );
    };
    (@expand ($consumer:path) [$($prefix:tt)*]) => {
        $consumer! {
            $($prefix)*
            export {
                Sources {
                    record: provenance_core::Source,
                    field: sources,
                    path: sources_path,
                    node: [Source],
                    reader: { open: list_sources, closed: [closed_sources],
                        strategy: direct },
                    id: field,
                    loader: [record(Source)],
                    graph: [export("source")],
                    import: [node_budget],
                    catalog: [projection(ListSourcesV2, "list-sources", PageSourcesV2,
                        "page-sources-v2", GetSourceV2, "get-source-v2")],
                    route: [order: 10, writable { mode: searchable,
                    plural: "sources",
                    singular: "source",
                    singular_id: "Source",
                    plural_id: "Sources",
                    create: CreateSource,
                    update: UpdateSource,
                    create_defaults: CREATE_SOURCE_DEFAULTS,
                    create_aliases: NO_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[("url", "url"), ("reference", "reference"),
                        ("commit_pin", "commit_pin"),
                        ("effective_date", "effective_date"),
                        ("review_date", "review_date")],
                    target: Some(NodeType::Source) }]
                };
                Domains {
                    record: provenance_core::Domain,
                    field: domains,
                    path: domains_path,
                    node: [Domain],
                    reader: { open: list_domains, closed: [closed_domains],
                        strategy: direct },
                    id: field,
                    loader: [record(Domain)],
                    graph: [export("domain")],
                    import: [node_budget],
                    catalog: [projection(ListDomainsV2, "list-domains", PageDomainsV2,
                        "page-domains-v2", GetDomainV2, "get-domain-v2")],
                    route: [order: 50, writable { mode: searchable,
                    plural: "domains",
                    singular: "domain",
                    singular_id: "Domain",
                    plural_id: "Domains",
                    create: CreateDomain,
                    update: UpdateDomain,
                    create_defaults: NONE,
                    create_aliases: NO_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[("description", "description"), ("color", "color")],
                    target: Some(NodeType::Domain) }]
                };
                Requirements {
                    record: provenance_core::Requirement,
                    field: requirements,
                    path: requirements_path,
                    node: [Requirement],
                    reader: { open: list_requirements, closed: [closed_requirements],
                        strategy: direct },
                    id: field,
                    loader: [record(Requirement)],
                    graph: [export("requirement")],
                    import: [node_budget],
                    catalog: [projection(ListRequirementsV2, "list-requirements",
                        PageRequirementsV2, "page-requirements-v2", none)],
                    route: [order: 20, requirements]
                };
                Boundaries {
                    record: provenance_core::Boundary,
                    field: boundaries,
                    path: boundaries_path,
                    node: [Boundary],
                    reader: { open: list_boundaries, closed: [closed_boundaries],
                        strategy: direct },
                    id: field,
                    loader: [record(Boundary)],
                    graph: [export("boundary")],
                    import: [node_budget],
                    catalog: [projection(ListBoundariesV2, "list-boundaries",
                        PageBoundariesV2, "page-boundaries-v2", GetBoundaryV2,
                        "get-boundary-v2")],
                    route: [order: 60, writable { mode: searchable,
                    plural: "boundaries",
                    singular: "boundary",
                    singular_id: "Boundary",
                    plural_id: "Boundaries",
                    create: CreateBoundary,
                    update: UpdateBoundary,
                    create_defaults: NONE,
                    create_aliases: NO_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[("source_ref", "source_ref")],
                    target: Some(NodeType::Boundary) }]
                };
                Topics {
                    record: provenance_core::Topic,
                    field: topics,
                    path: topics_path,
                    node: [Topic],
                    reader: { open: list_topics, closed: [closed_topics], strategy: direct },
                    id: field,
                    loader: [record(Topic)],
                    graph: [export("topic")],
                    import: [node_budget],
                    catalog: [projection(ListTopicsV2, "list-topics", PageTopicsV2,
                        "page-topics-v2", GetTopicV2, "get-topic-v2")],
                    route: [order: 70, writable { mode: searchable,
                    plural: "topics",
                    singular: "topic",
                    singular_id: "Topic",
                    plural_id: "Topics",
                    create: CreateTopic,
                    update: UpdateTopic,
                    create_defaults: OPEN_LINK_DEFAULTS,
                    create_aliases: NO_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[],
                    target: Some(NodeType::Topic) }]
                };
                Questions {
                    record: provenance_core::Question,
                    field: questions,
                    path: questions_path,
                    node: [Question],
                    reader: { open: list_questions, closed: [closed_questions],
                        strategy: direct },
                    id: field,
                    loader: [record(Question)],
                    graph: [export("question")],
                    import: [node_budget],
                    catalog: [projection(ListQuestionsV2, "list-questions", PageQuestionsV2,
                        "page-questions-v2", GetQuestionV2, "get-question-v2")],
                    route: [order: 80, writable { mode: searchable,
                    plural: "questions",
                    singular: "question",
                    singular_id: "Question",
                    plural_id: "Questions",
                    create: CreateQuestion,
                    update: UpdateQuestion,
                    create_defaults: OPEN_LINK_DEFAULTS,
                    create_aliases: QUESTION_ALIASES,
                    update_defaults: NONE,
                    update_aliases: QUESTION_ALIASES,
                    nullable: &[("resolution_id", "resolution_id"),
                        ("contradicts", "contradicts")],
                    target: Some(NodeType::Question) }]
                };
                Resolutions {
                    record: provenance_core::Resolution,
                    field: resolutions,
                    path: resolutions_path,
                    node: [Resolution],
                    reader: { open: list_resolutions, closed: [closed_resolutions],
                        strategy: direct },
                    id: field,
                    loader: [record(Resolution)],
                    graph: [export("resolution")],
                    import: [node_budget],
                    catalog: [projection(ListResolutionsV2, "list-resolutions",
                        PageResolutionsV2, "page-resolutions-v2", GetResolutionV2,
                        "get-resolution-v2")],
                    route: [order: 30, writable { mode: searchable,
                    plural: "resolutions",
                    singular: "resolution",
                    singular_id: "Resolution",
                    plural_id: "Resolutions",
                    create: CreateResolution,
                    update: UpdateResolution,
                    create_defaults: CREATE_RESOLUTION_DEFAULTS,
                    create_aliases: RESOLUTION_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[("context", "context"),
                        ("enforcement", "enforcement"),
                        ("confidence", "confidence"), ("made_by", "made_by"),
                        ("approved_by", "approved_by"),
                        ("approved_at", "approved_at"), ("review_on", "review_on")],
                    target: Some(NodeType::Resolution) }]
                };
                Rules {
                    record: provenance_core::Rule,
                    field: rules,
                    path: rules_path,
                    node: [Rule],
                    reader: { open: list_rules, closed: [closed_rules], strategy: rules },
                    id: field,
                    loader: [record(Rule)],
                    graph: [export("rule")],
                    import: [node_budget],
                    catalog: [projection(ListRulesV2, "list-rules", PageRulesV2,
                        "page-rules-v2", GetRuleV2, "get-rule-v2")],
                    route: [order: 40, writable { mode: rules,
                    plural: "rules",
                    singular: "rule",
                    singular_id: "Rule",
                    plural_id: "Rules",
                    create: CreateRule,
                    update: UpdateRule,
                    create_defaults: CREATE_RULE_DEFAULTS,
                    create_aliases: RULE_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[("name", "name"), ("description", "description"),
                        ("source_document", "source_document"),
                        ("source_section", "source_section")],
                    target: Some(NodeType::Rule) }]
                };
            }
            canonical {
                Threads {
                    record: provenance_core::Thread,
                    field: threads,
                    path: threads_path,
                    node: [],
                    reader: { open: list_threads, closed: [], strategy: direct },
                    id: field,
                    loader: [payload(load_threads)],
                    graph: [],
                    import: [threads_assignable_budget],
                    catalog: [payload(ListDiscussionContainersV2,
                        "list-discussion-containers", PageDiscussionContainersV2,
                        "page-discussion-containers-v2", GetDiscussionContainerV2,
                        "get-discussion-container-v2")],
                    route: [order: 140, read { mode: plain,
                    plural: "discussion-containers",
                    singular: "discussion-container",
                    singular_id: "DiscussionContainer",
                    plural_id: "DiscussionContainers" }]
                };
                Messages {
                    record: provenance_core::Message,
                    field: messages,
                    path: messages_path,
                    node: [],
                    reader: { open: list_messages, closed: [], strategy: messages },
                    id: field,
                    loader: [payload(load_messages)],
                    graph: [],
                    import: [messages_assignable_budget],
                    catalog: [payload(ListMessagesV2, "list-messages-v2", PageMessagesV2,
                        "page-messages-v2", GetMessageV2, "get-message-v2")],
                    route: [order: 150, read { mode: plain,
                    plural: "messages",
                    singular: "message",
                    singular_id: "Message",
                    plural_id: "Messages" }]
                };
                Contributions {
                    record: provenance_core::Contribution,
                    field: contributions,
                    path: contributions_path,
                    node: [],
                    reader: { open: list_contributions, closed: [],
                        strategy: contributions },
                    id: field,
                    loader: [payload(load_contributions)],
                    graph: [],
                    import: [assignable_budget],
                    catalog: [payload(ListContributionsV2, "list-contributions",
                        PageContributionsV2, "page-contributions-v2", GetContributionV2,
                        "get-contribution-v2")],
                    route: [order: 90, writable { mode: plain,
                    plural: "contributions",
                    singular: "contribution",
                    singular_id: "Contribution",
                    plural_id: "Contributions",
                    create: CreateContribution,
                    update: UpsertContribution,
                    create_defaults: NONE,
                    create_aliases: NO_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[],
                    target: None }]
                };
                SynthesisPackets {
                    record: provenance_core::SynthesisPacket,
                    field: synthesis_packets,
                    path: synthesis_packets_path,
                    node: [],
                    reader: { open: list_synthesis_packets, closed: [],
                        strategy: synthesis_packets },
                    id: field,
                    loader: [payload(load_synthesis_packets)],
                    graph: [],
                    import: [assignable_budget],
                    catalog: [payload(ListSynthesisPacketsV2, "list-synthesis-packets",
                        PageSynthesisPacketsV2, "page-synthesis-packets-v2",
                        GetSynthesisPacketV2, "get-synthesis-packet-v2")],
                    route: [order: 100, writable { mode: plain,
                    plural: "synthesis-packets",
                    singular: "synthesis-packet",
                    singular_id: "SynthesisPacket",
                    plural_id: "SynthesisPackets",
                    create: CreateSynthesisPacket,
                    update: UpsertSynthesisPacket,
                    create_defaults: NONE,
                    create_aliases: NO_ALIASES,
                    update_defaults: NONE,
                    update_aliases: NO_ALIASES,
                    nullable: &[],
                    target: None }]
                };
                ProposalCards {
                    record: provenance_core::ProposalCard,
                    field: proposal_cards,
                    path: proposal_cards_path,
                    node: [],
                    reader: { open: list_proposal_cards, closed: [],
                        strategy: proposal_cards },
                    id: field,
                    loader: [payload(load_proposal_cards)],
                    graph: [],
                    import: [assignable_budget],
                    catalog: [payload(ListProposalsV2, "list-proposals-v2", PageProposalsV2,
                        "page-proposals-v2", GetProposalV2, "get-proposal-v2")],
                    route: [order: 110, proposal { plural: "proposals",
                    singular: "proposal",
                    singular_id: "Proposal",
                    plural_id: "Proposals" }]
                };
                AssertionRecords {
                    record: provenance_core::AssertionRecord,
                    field: assertion_records,
                    path: assertion_records_path,
                    node: [],
                    reader: { open: list_assertion_records, closed: [],
                        strategy: assertion_records },
                    id: field,
                    loader: [payload(load_assertion_records)],
                    graph: [],
                    import: [assignable_budget],
                    catalog: [payload(ListAssertionsV2, "list-assertions-v2", PageAssertionsV2,
                        "page-assertions-v2", GetAssertionV2, "get-assertion-v2")],
                    route: [order: 160, read { mode: plain,
                    plural: "assertions",
                    singular: "assertion",
                    singular_id: "Assertion",
                    plural_id: "Assertions" }]
                };
                Dispositions {
                    record: provenance_core::DispositionRecord,
                    field: dispositions,
                    path: dispositions_path,
                    node: [],
                    reader: { open: list_dispositions, closed: [], strategy: dispositions },
                    id: field,
                    loader: [payload(load_dispositions)],
                    graph: [],
                    import: [assignable_budget],
                    catalog: [payload(ListDispositionsV2, "list-dispositions-v2",
                        PageDispositionsV2, "page-dispositions-v2", GetDispositionV2,
                        "get-disposition-v2")],
                    route: [order: 170, read { mode: plain,
                    plural: "dispositions",
                    singular: "disposition",
                    singular_id: "Disposition",
                    plural_id: "Dispositions" }]
                };
            }
            bindings {
                ImplementationBindings {
                    record: provenance_core::ImplementationBinding,
                    field: implementation_bindings,
                    path: implementation_bindings_path,
                    node: [],
                    reader: { open: list_implementation_bindings,
                        closed: [closed_implementation_bindings], strategy: direct },
                    id: field,
                    loader: [kind],
                    graph: [implementation("implementation binding")],
                    import: [assignable],
                    catalog: [none],
                    route: [none]
                };
                VerificationBindings {
                    record: provenance_core::VerificationBinding,
                    field: verification_bindings,
                    path: verification_bindings_path,
                    node: [],
                    reader: { open: list_verification_bindings,
                        closed: [closed_verification_bindings], strategy: direct },
                    id: field,
                    loader: [kind],
                    graph: [verification("verification binding")],
                    import: [assignable_budget],
                    catalog: [verification(ListVerificationBindingsV2,
                        "list-verification-bindings", PageVerificationBindingsV2,
                        GetVerificationBindingV2, "get-verification-binding-v2")],
                    route: [order: 130, read { mode: verification,
                    plural: "verification-bindings",
                    singular: "verification-binding",
                    singular_id: "VerificationBinding",
                    plural_id: "VerificationBindings" }]
                };
            }
            internal {
                RequirementReviews {
                    record: provenance_core::RequirementReview,
                    field: requirement_reviews,
                    path: requirement_reviews_path,
                    node: [],
                    reader: { open: list_requirement_reviews, closed: [], strategy: existing },
                    id: field,
                    loader: [kind],
                    graph: [],
                    import: [skip],
                    catalog: [none],
                    route: [none]
                };
                ReviewJournal {
                    record: provenance_core::review::JournalEntry,
                    field: review_journal,
                    path: review_journal_path,
                    node: [],
                    reader: { open: validated_journal_entries, closed: [], strategy: existing },
                    id: method,
                    loader: [journal],
                    graph: [],
                    import: [skip],
                    catalog: [none],
                    route: [none]
                };
            }
        }
    };
}

pub(crate) use record_family_rows;
