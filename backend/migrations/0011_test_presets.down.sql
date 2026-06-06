-- 0011_test_presets (reverse): drop the test_presets table (its index drops with it).
DROP INDEX IF EXISTS rre.test_presets_created_at_idx;
DROP TABLE rre.test_presets;
