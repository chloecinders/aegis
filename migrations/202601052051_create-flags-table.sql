CREATE TABLE
    IF NOT EXISTS public.user_flags (
        user_id TEXT NOT NULL,
        flag TEXT NOT NULL,
        value BIGINT NOT NULL,
        expires_at TIMESTAMPTZ,
        PRIMARY KEY (user_id, flag)
    );
