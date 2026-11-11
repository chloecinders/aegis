CREATE TABLE public.message_images (
    message_id bigint NOT NULL,
    guild_id bigint NOT NULL,
    name text NOT NULL,
    token text NOT NULL,
    url text NOT NULL,
    encrypted boolean NOT NULL,
    hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX message_images_message_idx ON public.message_images (message_id);
CREATE INDEX message_images_token_idx ON public.message_images (token);
CREATE INDEX message_images_hash_idx ON public.message_images (guild_id, hash);

ALTER TABLE public.message_deletions
    ADD COLUMN log_channel_id bigint,
    ADD COLUMN log_message_id bigint,
    ADD COLUMN log_images integer;
