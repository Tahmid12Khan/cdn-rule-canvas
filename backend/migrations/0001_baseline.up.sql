-- 0001_baseline: schema, extensions, and shared enum types (BACKEND CONTRACT §3).
CREATE SCHEMA IF NOT EXISTS rre;

-- gen_random_uuid() safety net for UUID PK defaults.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE rre.feature_type   AS ENUM ('html', 'json');
CREATE TYPE rre.version_status AS ENUM ('draft', 'staging', 'live', 'prev');
CREATE TYPE rre.placement      AS ENUM ('inline', 'sticky_footer', 'popup');
