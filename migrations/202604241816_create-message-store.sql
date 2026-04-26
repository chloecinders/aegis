CREATE TABLE
    IF NOT EXISTS public.message_store (
        message_id bigint NOT NULL PRIMARY KEY,
        channel_id bigint NOT NULL,
        guild_id bigint NOT NULL,
        author_id bigint NOT NULL,
        author_name text NOT NULL,
        content bytea,
        attachment_urls jsonb,
        created_at timestamptz NOT NULL DEFAULT now ()
    );
