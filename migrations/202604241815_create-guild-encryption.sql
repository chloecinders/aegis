CREATE TABLE
    IF NOT EXISTS public.guild_encryption (
        guild_id bigint NOT NULL PRIMARY KEY,
        encrypted boolean NOT NULL DEFAULT false,
        key_channel_id bigint,
        key_message_id bigint
    );
