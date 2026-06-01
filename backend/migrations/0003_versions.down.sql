-- 0003_versions (reverse): drop deferrable FKs on features, then versions.
ALTER TABLE rre.features DROP CONSTRAINT features_staging_version_fk,
                         DROP CONSTRAINT features_live_version_fk;
DROP TABLE rre.versions;
