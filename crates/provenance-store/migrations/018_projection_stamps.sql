-- Projection stamps: tables for the three binding and review families the
-- served read path projects, plus the revision stamp and per-family digest
-- baseline the catch-up sweep compares against.
CREATE TABLE implementation_bindings (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    declared_by TEXT NOT NULL,
    retired INTEGER NOT NULL DEFAULT 0,
    file TEXT NOT NULL,
    symbol TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_implementation_bindings_scope_rule ON implementation_bindings (scope_id, rule_id);

CREATE TABLE verification_bindings (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    key TEXT NOT NULL,
    method TEXT NOT NULL,
    declared_by TEXT NOT NULL,
    retired INTEGER NOT NULL DEFAULT 0,
    file TEXT NOT NULL,
    symbol TEXT,
    payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_verification_bindings_scope_rule ON verification_bindings (scope_id, rule_id);

CREATE TABLE requirement_reviews (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    field TEXT NOT NULL,
    changed_at INTEGER NOT NULL,
    cleared_at INTEGER,
    payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX idx_requirement_reviews_scope_rule ON requirement_reviews (scope_id, rule_id);

CREATE TABLE projection_revision (
    serial INTEGER NOT NULL,
    instance_id TEXT NOT NULL,
    digest TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE projection_family_digests (
    scope_id TEXT NOT NULL,
    family TEXT NOT NULL,
    digest TEXT NOT NULL,
    record_count INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    PRIMARY KEY (scope_id, family)
);
