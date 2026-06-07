-- 0012_feature_execution_order: per-feature execution order (spec items 6 & 9).
--
-- Lowest `execution_order` runs first. The number space is PER `type`
-- (html/json independently), so an HTML feature and a JSON feature can both be
-- order 1. Within a single type the order is unique.
--
-- Additive add-then-backfill-then-constrain so existing rows get a deterministic
-- order BEFORE the unique constraint is enforced (a NOT NULL add on an existing
-- table needs a default; we backfill, then drop the default in the same txn).

-- 1. Add the column with a temporary default so existing rows are NOT NULL.
ALTER TABLE rre.features
    ADD COLUMN execution_order INTEGER NOT NULL DEFAULT 0;

-- 2. Backfill existing rows deterministically, numbering per type starting at 1
--    in (created_at, id) order — the historical list ordering — so re-running is
--    stable and no two same-type rows collide on the unique constraint below.
WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (PARTITION BY "type" ORDER BY created_at, id) AS rn
    FROM rre.features
)
UPDATE rre.features f
SET execution_order = ranked.rn
FROM ranked
WHERE f.id = ranked.id;

-- 3. Drop the default — new rows get an explicit order from the service
--    (auto-assigned next-free per type, or a caller-supplied value).
ALTER TABLE rre.features
    ALTER COLUMN execution_order DROP DEFAULT;

-- 4. Enforce uniqueness per type. An HTML feature and a JSON feature may share a
--    number; two same-type features may not. The service maps a violation here to
--    409 EXECUTION_ORDER_CONFLICT.
ALTER TABLE rre.features
    ADD CONSTRAINT features_type_execution_order_unique UNIQUE ("type", execution_order);
