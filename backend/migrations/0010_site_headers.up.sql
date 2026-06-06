-- 0010_site_headers: per-site custom request headers (additive).
-- The proxy injects these headers when forwarding to the destination; a
-- configured header OVERRIDES any client-supplied same-named header (an
-- anti-spoofing measure). Stored as a JSONB object `{ "Header-Name": "value" }`;
-- existing rows backfill to the empty map.
ALTER TABLE rre.sites ADD COLUMN headers JSONB NOT NULL DEFAULT '{}'::jsonb;
