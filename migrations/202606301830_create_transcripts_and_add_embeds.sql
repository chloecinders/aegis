ALTER TABLE public.message_store
ADD COLUMN IF NOT EXISTS embeds bytea;

CREATE TABLE
    IF NOT EXISTS public.transcripts (
        transcript_id text NOT NULL PRIMARY KEY,
        guild_id bigint NOT NULL,
        channel_id bigint NOT NULL,
        moderator_name text NOT NULL,
        message_ids jsonb NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now ()
    );
