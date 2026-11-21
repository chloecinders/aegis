ALTER TABLE public.guild_settings
    ADD COLUMN block_self_reactions boolean NOT NULL DEFAULT false;
