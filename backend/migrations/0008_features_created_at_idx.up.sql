-- 0008_features_created_at_idx: index the feature list ordering.
--
-- `feature_repository::list_paged` sorts by `created_at DESC, id ASC`; this
-- composite index lets Postgres satisfy that ORDER BY (and the paginated LIMIT)
-- without a full-table sort.
CREATE INDEX features_created_at_idx ON rre.features (created_at DESC, id ASC);
