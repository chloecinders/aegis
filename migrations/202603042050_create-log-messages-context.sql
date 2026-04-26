CREATE TABLE
    IF NOT EXISTS public.log_messages_context (
        message_id bigint NOT NULL PRIMARY KEY,
        guild_id bigint NOT NULL,
        target_id bigint NOT NULL,
        moderator_id bigint NOT NULL,
        db_id character varying(128) COLLATE pg_catalog."default",
        content text
    );
