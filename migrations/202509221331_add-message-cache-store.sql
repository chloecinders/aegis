CREATE TABLE
    IF NOT EXISTS public.message_cache_store (
        channel_id bigint NOT NULL,
        message_count integer NOT NULL DEFAULT 0,
        previous_action smallint NOT NULL DEFAULT 0 CHECK (previous_action BETWEEN -1 AND 1),
        PRIMARY KEY (channel_id)
    );
