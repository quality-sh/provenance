DROP TABLE IF EXISTS sources;
DROP TABLE IF EXISTS requirements;
DROP TABLE IF EXISTS resolutions;
DROP TABLE IF EXISTS rules;
DROP TABLE IF EXISTS boundaries;
DROP TABLE IF EXISTS topics;
DROP TABLE IF EXISTS questions;
DROP TABLE IF EXISTS domains;
DROP TABLE IF EXISTS implementation_bindings;
DROP TABLE IF EXISTS verification_bindings;
DROP TABLE IF EXISTS requirement_reviews;
DROP TABLE IF EXISTS relations;

CREATE TABLE sources (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    declared_by TEXT,
    declaration_address TEXT,
    retired INTEGER NOT NULL,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,
    url TEXT,
    reference TEXT,
    commit_pin TEXT,
    effective_date INTEGER,
    review_date INTEGER,
    supersedes TEXT NOT NULL,
    origin_thread TEXT,
    origin_message TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_sources_commit_pin ON sources(scope_id, commit_pin);

CREATE TABLE requirements (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    declared_by TEXT,
    declaration_address TEXT,
    retired INTEGER NOT NULL,
    statement TEXT NOT NULL,
    description TEXT,
    fog TEXT,
    status TEXT NOT NULL,
    domain_id TEXT,
    source_refs TEXT NOT NULL,
    refines TEXT,
    depends_on TEXT NOT NULL,
    supersedes TEXT NOT NULL,
    spawned_by TEXT,
    origin_thread TEXT,
    origin_message TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);

CREATE TABLE resolutions (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    title TEXT NOT NULL,
    position TEXT NOT NULL,
    rationale TEXT NOT NULL,
    status TEXT NOT NULL,
    context TEXT,
    enforcement TEXT,
    confidence REAL,
    inputs TEXT NOT NULL,
    made_by TEXT,
    approved_by TEXT,
    approved_at INTEGER,
    requirement_ids TEXT NOT NULL,
    supersedes TEXT NOT NULL,
    review_on TEXT,
    origin_thread TEXT,
    origin_message TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_resolutions_status_review ON resolutions(scope_id, status, review_on);

CREATE TABLE rules (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    declared_by TEXT,
    declaration_address TEXT,
    retired INTEGER NOT NULL,
    name TEXT,
    description TEXT,
    statement TEXT NOT NULL,
    status TEXT NOT NULL,
    severity TEXT NOT NULL,
    requirement_ids TEXT NOT NULL,
    resolution_ids TEXT NOT NULL,
    source_document TEXT,
    source_section TEXT,
    origin_thread TEXT,
    origin_message TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_rules_status_severity ON rules(scope_id, status, severity);

CREATE TABLE boundaries (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    statement TEXT NOT NULL,
    source_ref TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_boundaries_requirement ON boundaries(scope_id, requirement_id);

CREATE TABLE topics (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    claimed_by TEXT,
    claimed_at INTEGER,
    links TEXT NOT NULL,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_topics_requirement ON topics(scope_id, requirement_id);
CREATE INDEX idx_topics_status ON topics(scope_id, status);
CREATE INDEX idx_topics_claimed_by ON topics(scope_id, claimed_by);

CREATE TABLE questions (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    question TEXT NOT NULL,
    resolution_method TEXT NOT NULL,
    status TEXT NOT NULL,
    claimed_by TEXT,
    claimed_at INTEGER,
    answer TEXT,
    links TEXT NOT NULL,
    resolution_id TEXT,
    contradicts TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_questions_topic ON questions(scope_id, topic_id);
CREATE INDEX idx_questions_requirement ON questions(scope_id, requirement_id);
CREATE INDEX idx_questions_status ON questions(scope_id, status);
CREATE INDEX idx_questions_claimed_by ON questions(scope_id, claimed_by);

CREATE TABLE domains (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    color TEXT,
    search_text TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_domains_name ON domains(scope_id, name);

CREATE TABLE implementation_bindings (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    declared_by TEXT NOT NULL,
    retired INTEGER NOT NULL,
    file TEXT NOT NULL,
    symbol TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_implementation_bindings_rule ON implementation_bindings(scope_id, rule_id);

CREATE TABLE verification_bindings (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    "key" TEXT NOT NULL,
    method TEXT NOT NULL,
    declared_by TEXT NOT NULL,
    retired INTEGER NOT NULL,
    file TEXT NOT NULL,
    symbol TEXT,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_verification_bindings_rule ON verification_bindings(scope_id, rule_id);

CREATE TABLE requirement_reviews (
    schema_version INTEGER NOT NULL,
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    "field" TEXT NOT NULL,
    "before" TEXT NOT NULL,
    "after" TEXT NOT NULL,
    changed_at INTEGER NOT NULL,
    cleared_at INTEGER,
    cleared_by_run TEXT,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_requirement_reviews_rule ON requirement_reviews(scope_id, rule_id);

CREATE TABLE relations (
    scope_id TEXT NOT NULL,
    owner_type TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    PRIMARY KEY (scope_id, owner_type, owner_id, relation, target_type, target_id)
);
CREATE INDEX idx_relations_out ON relations(scope_id, owner_type, owner_id, relation);
CREATE INDEX idx_relations_in ON relations(scope_id, target_type, target_id, relation);
