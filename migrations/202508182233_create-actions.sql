CREATE TABLE
    IF NOT EXISTS public.actions (
        guild_id bigint NOT NULL,
        user_id bigint NOT NULL,
        reason text COLLATE pg_catalog."default" NOT NULL,
        moderator_id bigint NOT NULL,
        created_at timestamp without time zone NOT NULL DEFAULT now (),
        updated_at timestamp without time zone,
        id character varying(128) COLLATE pg_catalog."default" NOT NULL,
        type action_type NOT NULL DEFAULT 'warn',
        active boolean NOT NULL DEFAULT true,
        expires_at timestamp without time zone,
        CONSTRAINT warns_pkey PRIMARY KEY (id)
    )
