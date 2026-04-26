CREATE TABLE
    IF NOT EXISTS public.guild_settings (
        guild_id bigint NOT NULL PRIMARY KEY,
        log_channel bigint
    )
