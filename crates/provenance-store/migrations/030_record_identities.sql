CREATE TABLE record_identities (
    scope_id TEXT NOT NULL,
    id TEXT NOT NULL,
    node_type TEXT NOT NULL,
    PRIMARY KEY (scope_id, node_type, id)
);
CREATE UNIQUE INDEX idx_record_identities_id ON record_identities(id);
