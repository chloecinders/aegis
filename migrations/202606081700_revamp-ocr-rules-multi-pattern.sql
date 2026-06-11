ALTER TABLE automod_rules
ADD COLUMN IF NOT EXISTS patterns JSONB;

UPDATE automod_rules
SET
    patterns = jsonb_build_array (
        jsonb_build_object ('pattern', rule, 'is_regex', is_regex)
    )
WHERE
    patterns IS NULL;

CREATE TABLE
    IF NOT EXISTS ocr_image_hashes (
        image_hash CHAR(64) NOT NULL,
        rule_id VARCHAR(128) NOT NULL,
        guild_id BIGINT NOT NULL,
        is_regex BOOLEAN NOT NULL,
        matched_pattern TEXT NOT NULL,
        matched_at TIMESTAMPTZ NOT NULL DEFAULT now (),
        PRIMARY KEY (image_hash, rule_id)
    );

CREATE INDEX IF NOT EXISTS ocr_image_hashes_lookup_idx ON ocr_image_hashes (guild_id, image_hash);
