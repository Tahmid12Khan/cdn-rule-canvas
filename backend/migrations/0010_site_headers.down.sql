-- 0010_site_headers (reverse): drop the per-site custom headers column.
ALTER TABLE rre.sites DROP COLUMN headers;
