CREATE TABLE IF NOT EXISTS projection_unit_digests (
    unit TEXT PRIMARY KEY,
    digest TEXT NOT NULL
);
ALTER TABLE projection_family_digests DROP COLUMN digest;
ALTER TABLE projection_family_digests DROP COLUMN size_bytes;
ALTER TABLE projection_family_digests DROP COLUMN mtime_ns;
