CREATE TABLE IF NOT EXISTS implementation_bindings (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    declared_by TEXT NOT NULL,
    retired INTEGER NOT NULL DEFAULT 0,
    file TEXT NOT NULL,
    symbol TEXT NOT NULL,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX IF NOT EXISTS idx_implementation_bindings_rule
    ON implementation_bindings(scope_id, rule_id);

CREATE TABLE IF NOT EXISTS verification_bindings (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    key TEXT NOT NULL,
    method TEXT NOT NULL,
    declared_by TEXT NOT NULL,
    retired INTEGER NOT NULL DEFAULT 0,
    file TEXT NOT NULL,
    symbol TEXT,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX IF NOT EXISTS idx_verification_bindings_rule
    ON verification_bindings(scope_id, rule_id);

CREATE TABLE IF NOT EXISTS requirement_reviews (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    field TEXT NOT NULL,
    before_text TEXT NOT NULL,
    after_text TEXT NOT NULL,
    changed_at INTEGER NOT NULL,
    cleared_at INTEGER,
    cleared_by_run TEXT,
    PRIMARY KEY (scope_id, id)
);
CREATE INDEX IF NOT EXISTS idx_requirement_reviews_rule
    ON requirement_reviews(scope_id, rule_id);

CREATE TABLE IF NOT EXISTS projection_instance (
    only_row INTEGER PRIMARY KEY CHECK (only_row = 1),
    instance_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projection_revision (
    serial INTEGER PRIMARY KEY,
    digest TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS projection_family_digests (
    scope_id TEXT NOT NULL,
    family TEXT NOT NULL,
    digest TEXT NOT NULL,
    record_count INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    PRIMARY KEY (scope_id, family)
);
