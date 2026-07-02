CREATE TABLE
    IF NOT EXISTS public.sticky_messages (
        channel_id bigint PRIMARY KEY NOT NULL,
        content text NOT NULL,
        title text,
        color bigint,
        last_message_id bigint
    );
