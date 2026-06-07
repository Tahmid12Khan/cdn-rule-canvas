-- Reverse of 0012_feature_execution_order: drop the unique constraint and the
-- execution_order column.
ALTER TABLE rre.features
    DROP CONSTRAINT IF EXISTS features_type_execution_order_unique;

ALTER TABLE rre.features
    DROP COLUMN IF EXISTS execution_order;
