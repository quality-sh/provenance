-- Applied as migration 027 because 007 is already present in the migration history.
ALTER TABLE sources ADD COLUMN created TEXT;
ALTER TABLE sources ADD COLUMN updated TEXT;
ALTER TABLE requirements ADD COLUMN created TEXT;
ALTER TABLE requirements ADD COLUMN updated TEXT;
ALTER TABLE rules ADD COLUMN created TEXT;
ALTER TABLE rules ADD COLUMN updated TEXT;
ALTER TABLE rules ADD COLUMN archived_in_commit TEXT;
ALTER TABLE resolutions ADD COLUMN created TEXT;
ALTER TABLE resolutions ADD COLUMN updated TEXT;
ALTER TABLE projection_unit_digests ADD COLUMN stored_digest TEXT NOT NULL DEFAULT '';
