CREATE TABLE IF NOT EXISTS public.action_refs
        (
            action_id character varying(128) COLLATE pg_catalog."default" NOT NULL,
            message_content text,
            image_url text,
            CONSTRAINT action_refs_pkey PRIMARY KEY (action_id)
        );
