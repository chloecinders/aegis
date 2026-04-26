CREATE TABLE
    IF NOT EXISTS public.message_edits (
        edit_id bigserial PRIMARY KEY,
        message_id bigint NOT NULL,
        content bytea,
        created_at timestamptz NOT NULL DEFAULT now ()
    );
