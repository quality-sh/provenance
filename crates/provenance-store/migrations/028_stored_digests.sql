ALTER TABLE projection_unit_digests ADD COLUMN stored_digest TEXT NOT NULL DEFAULT '';
ALTER TABLE projection_family_digests ADD COLUMN stored_digest TEXT NOT NULL DEFAULT '';
