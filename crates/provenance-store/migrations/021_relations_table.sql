DROP INDEX IF EXISTS idx_edges_from;
DROP INDEX IF EXISTS idx_edges_to;
DROP INDEX IF EXISTS idx_edges_scope_type_from;
DROP INDEX IF EXISTS idx_edges_scope_type_to;
DROP TABLE IF EXISTS edges;
DELETE FROM projection_family_digests WHERE family = 'edges';
ALTER TABLE sources DROP COLUMN superseded_by;
ALTER TABLE resolutions DROP COLUMN superseded_by;
CREATE TABLE relations (
    scope_id TEXT NOT NULL,
    owner_type TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    PRIMARY KEY (scope_id, owner_type, owner_id, relation, target_id)
);
CREATE INDEX idx_relations_out ON relations(scope_id, owner_type, owner_id, relation);
CREATE INDEX idx_relations_in ON relations(scope_id, target_type, target_id, relation);
