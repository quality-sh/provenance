CREATE TABLE _cache_metadata (
    only_row INTEGER PRIMARY KEY CHECK (only_row = 1),
    schema_digest TEXT NOT NULL
);

CREATE TABLE sources (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    declared_by TEXT, declaration_address TEXT, name TEXT NOT NULL,
    source_type TEXT NOT NULL, url TEXT, reference TEXT, commit_pin TEXT,
    effective_date INTEGER, review_date INTEGER, supersedes TEXT NOT NULL,
    origin_thread TEXT, origin_message TEXT, search_text TEXT NOT NULL,
    created TEXT, updated TEXT, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_sources_commit_pin ON sources(scope_id, commit_pin);

CREATE TABLE requirements (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    declared_by TEXT, declaration_address TEXT, statement TEXT NOT NULL,
    description TEXT, fog TEXT, status TEXT NOT NULL, domain_id TEXT,
    source_refs TEXT NOT NULL, refines TEXT, depends_on TEXT NOT NULL,
    supersedes TEXT NOT NULL, spawned_by TEXT, origin_thread TEXT,
    origin_message TEXT, search_text TEXT NOT NULL, created TEXT, updated TEXT,
    PRIMARY KEY (scope_id, id)
);

CREATE TABLE resolutions (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    title TEXT NOT NULL, position TEXT NOT NULL, rationale TEXT NOT NULL,
    status TEXT NOT NULL, context TEXT, enforcement TEXT, confidence REAL,
    inputs TEXT NOT NULL, made_by TEXT, approved_by TEXT, approved_at INTEGER,
    requirement_ids TEXT NOT NULL, supersedes TEXT NOT NULL, review_on TEXT,
    origin_thread TEXT, origin_message TEXT, search_text TEXT NOT NULL,
    created TEXT, updated TEXT, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_resolutions_status_review ON resolutions(scope_id, status, review_on);

CREATE TABLE rules (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    declared_by TEXT, declaration_address TEXT, name TEXT, description TEXT,
    statement TEXT NOT NULL, status TEXT NOT NULL, severity TEXT NOT NULL,
    requirement_ids TEXT NOT NULL, resolution_ids TEXT NOT NULL,
    source_document TEXT, source_section TEXT, origin_thread TEXT,
    origin_message TEXT, search_text TEXT NOT NULL, created TEXT, updated TEXT,
    archived_in_commit TEXT, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_rules_status_severity ON rules(scope_id, status, severity);

CREATE TABLE boundaries (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    requirement_id TEXT NOT NULL, statement TEXT NOT NULL, source_ref TEXT,
    search_text TEXT NOT NULL, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_boundaries_requirement ON boundaries(scope_id, requirement_id);

CREATE TABLE topics (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    requirement_id TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL,
    archived_in_commit TEXT, claimed_by TEXT, claimed_at INTEGER, links TEXT NOT NULL,
    search_text TEXT NOT NULL, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_topics_requirement ON topics(scope_id, requirement_id);
CREATE INDEX idx_topics_status ON topics(scope_id, status);
CREATE INDEX idx_topics_claimed_by ON topics(scope_id, claimed_by);

CREATE TABLE questions (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    topic_id TEXT NOT NULL, requirement_id TEXT NOT NULL, question TEXT NOT NULL,
    resolution_method TEXT NOT NULL, status TEXT NOT NULL, claimed_by TEXT,
    archived_in_commit TEXT, claimed_at INTEGER, answer TEXT, links TEXT NOT NULL, resolution_id TEXT,
    contradicts TEXT, search_text TEXT NOT NULL, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_questions_topic ON questions(scope_id, topic_id);
CREATE INDEX idx_questions_requirement ON questions(scope_id, requirement_id);
CREATE INDEX idx_questions_status ON questions(scope_id, status);
CREATE INDEX idx_questions_claimed_by ON questions(scope_id, claimed_by);

CREATE TABLE domains (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    name TEXT NOT NULL, description TEXT, color TEXT, search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_domains_name ON domains(scope_id, name);

CREATE TABLE implementation_bindings (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    rule_id TEXT NOT NULL, declared_by TEXT NOT NULL, file TEXT NOT NULL,
    symbol TEXT NOT NULL, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_implementation_bindings_rule ON implementation_bindings(scope_id, rule_id);

CREATE TABLE verification_bindings (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    rule_id TEXT NOT NULL, "key" TEXT NOT NULL, method TEXT NOT NULL,
    declared_by TEXT NOT NULL, file TEXT NOT NULL, symbol TEXT,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_verification_bindings_rule ON verification_bindings(scope_id, rule_id);

CREATE TABLE requirement_reviews (
    schema_version INTEGER NOT NULL, scope_id TEXT NOT NULL, id TEXT NOT NULL,
    rule_id TEXT NOT NULL, requirement_id TEXT NOT NULL, "field" TEXT NOT NULL,
    "before" TEXT NOT NULL, "after" TEXT NOT NULL, changed_at INTEGER NOT NULL,
    cleared_at INTEGER, cleared_by_run TEXT, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_requirement_reviews_rule ON requirement_reviews(scope_id, rule_id);

CREATE TABLE relations (
    scope_id TEXT NOT NULL, owner_type TEXT NOT NULL, owner_id TEXT NOT NULL,
    relation TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL,
    PRIMARY KEY (scope_id, owner_type, owner_id, relation, target_type, target_id)
);
CREATE INDEX idx_relations_out ON relations(scope_id, owner_type, owner_id, relation);
CREATE INDEX idx_relations_in ON relations(scope_id, target_type, target_id, relation);

CREATE TABLE threads (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, parent_type TEXT NOT NULL,
    parent_id TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL,
    payload TEXT NOT NULL, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_threads_parent_status ON threads(scope_id, parent_type, parent_id, status);

CREATE TABLE messages (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, thread_id TEXT NOT NULL,
    role TEXT NOT NULL, body TEXT NOT NULL, created_at INTEGER NOT NULL,
    ai_metadata TEXT, payload TEXT NOT NULL, PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_messages_thread_order ON messages(scope_id, thread_id, created_at, id);

CREATE TABLE contributions (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, target_type TEXT NOT NULL,
    target_id TEXT NOT NULL, participant_slot TEXT NOT NULL, stance TEXT NOT NULL,
    strongest_finding TEXT NOT NULL, uncertainty TEXT NOT NULL, payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_contributions_target ON contributions(scope_id, target_type, target_id);

CREATE TABLE synthesis_packets (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, target_type TEXT NOT NULL,
    target_id TEXT NOT NULL, summary TEXT NOT NULL, payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_synthesis_packets_target ON synthesis_packets(scope_id, target_type, target_id);

CREATE TABLE proposal_cards (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, proposal_key TEXT NOT NULL,
    proposal_type TEXT NOT NULL, title TEXT NOT NULL, summary TEXT NOT NULL,
    target_type TEXT NOT NULL, target_id TEXT NOT NULL, traceability TEXT NOT NULL,
    promotion_state TEXT NOT NULL, duplicate_of TEXT, superseded_by TEXT,
    confidence REAL, builds_on TEXT NOT NULL, payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_proposal_cards_target ON proposal_cards(scope_id, target_type, target_id);
CREATE INDEX idx_proposal_cards_state ON proposal_cards(scope_id, promotion_state);

CREATE TABLE dispositions (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, proposal_id TEXT NOT NULL,
    decision TEXT NOT NULL, rationale TEXT NOT NULL, actor TEXT NOT NULL,
    canonical_artifact TEXT, external_action TEXT, payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_dispositions_proposal ON dispositions(scope_id, proposal_id);

CREATE TABLE assertion_records (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, proposal_id TEXT NOT NULL,
    synthesis_packet_id TEXT NOT NULL, supporting_claim_ids TEXT NOT NULL,
    payload TEXT NOT NULL, PRIMARY KEY (scope_id, id), UNIQUE (scope_id, proposal_id)
);

CREATE TABLE projection_instance (
    only_row INTEGER PRIMARY KEY CHECK (only_row = 1), instance_id TEXT NOT NULL
);
CREATE TABLE projection_revision (
    serial INTEGER PRIMARY KEY, digest TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE projection_family_digests (
    scope_id TEXT NOT NULL, family TEXT NOT NULL, record_count INTEGER NOT NULL,
    content_digest TEXT NOT NULL, stored_digest TEXT NOT NULL,
    PRIMARY KEY (scope_id, family)
);
CREATE TABLE projection_unit_digests (
    unit TEXT PRIMARY KEY, digest TEXT NOT NULL, stored_digest TEXT NOT NULL
);
CREATE TABLE projection_validation (
    only_row INTEGER PRIMARY KEY CHECK (only_row = 1), version INTEGER NOT NULL
);

CREATE TABLE review_journal (
    scope_id TEXT NOT NULL, kind TEXT NOT NULL, record_kind TEXT, record_id TEXT, id TEXT NOT NULL,
    sequence INTEGER, request_id TEXT NOT NULL, payload TEXT NOT NULL,
    discussion_id TEXT, thread_id TEXT, message_id TEXT, parent_type TEXT,
    parent_id TEXT, version INTEGER, PRIMARY KEY (scope_id, id),
    UNIQUE (scope_id, request_id), UNIQUE (scope_id, kind, record_kind, record_id, sequence),
    UNIQUE (scope_id, discussion_id, version), UNIQUE (scope_id, message_id)
);
CREATE INDEX discussion_parent ON review_journal(scope_id, parent_type, parent_id, discussion_id, version);
CREATE INDEX discussion_thread ON review_journal(scope_id, thread_id, message_id);
CREATE INDEX review_record ON review_journal(scope_id, record_kind, record_id, sequence);

CREATE TABLE record_identities (
    scope_id TEXT NOT NULL, id TEXT NOT NULL, node_type TEXT NOT NULL,
    PRIMARY KEY (scope_id, node_type, id)
);
CREATE UNIQUE INDEX idx_record_identities_id ON record_identities(id);
