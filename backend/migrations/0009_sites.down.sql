-- 0009_sites (reverse): drop the sites table (its indexes drop with it).
DROP INDEX IF EXISTS rre.sites_created_at_idx;
DROP INDEX IF EXISTS rre.sites_source_unique;
DROP TABLE rre.sites;
