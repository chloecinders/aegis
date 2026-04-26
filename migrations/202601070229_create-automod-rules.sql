CREATE TABLE
    IF NOT EXISTS public.automod_rules (
        id character varying(128) COLLATE pg_catalog."default" NOT NULL,
        guild_id bigint NOT NULL,
        name character varying(128) COLLATE pg_catalog."default" NOT NULL,
        type character varying(128) COLLATE pg_catalog."default" NOT NULL,
        rule character varying(512) COLLATE pg_catalog."default" NOT NULL,
        is_regex boolean NOT NULL,
        created_at timestamp without time zone NOT NULL DEFAULT now (),
        reason text COLLATE pg_catalog."default" NOT NULL,
        punishment_type action_type NOT NULL DEFAULT 'warn',
        duration BIGINT,
        day_clear_amount SMALLINT,
        silent BOOL,
        CONSTRAINT automod_rules_pkey PRIMARY KEY (id)
    )
