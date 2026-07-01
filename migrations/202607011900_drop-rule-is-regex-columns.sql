ALTER TABLE automod_rules
DROP COLUMN IF EXISTS patterns;

ALTER TABLE ocr_image_hashes
DROP COLUMN IF EXISTS is_regex,
DROP COLUMN IF EXISTS matched_pattern;
