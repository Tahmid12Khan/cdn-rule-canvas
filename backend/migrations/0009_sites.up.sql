-- 0009_sites: the rre.sites table — user-configurable source→destination routing.
-- A request arriving on a Site's source host:port is reverse-proxied to that
-- Site's destination protocol://host:port (the proxy matches the incoming Host
-- header against (source_host, source_port); it does not bind a port per Site).
CREATE TABLE rre.sites (
    slug            VARCHAR(64)  PRIMARY KEY,
    name            VARCHAR(200) NOT NULL UNIQUE,
    source_protocol VARCHAR(8)   NOT NULL CHECK (source_protocol IN ('http', 'https')),
    source_host     VARCHAR(255) NOT NULL,
    source_port     INTEGER      NOT NULL CHECK (source_port BETWEEN 1 AND 65535),
    dest_protocol   VARCHAR(8)   NOT NULL CHECK (dest_protocol IN ('http', 'https')),
    dest_host       VARCHAR(255) NOT NULL,
    dest_port       INTEGER      NOT NULL CHECK (dest_port BETWEEN 1 AND 65535),
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- Deterministic routing: at most one Site per (source_host, source_port).
CREATE UNIQUE INDEX sites_source_unique ON rre.sites (source_host, source_port);

-- List pagination: `site_repository::list_paged` sorts by `created_at DESC,
-- slug ASC` (mirror 0008_features_created_at_idx).
CREATE INDEX sites_created_at_idx ON rre.sites (created_at DESC, slug ASC);
