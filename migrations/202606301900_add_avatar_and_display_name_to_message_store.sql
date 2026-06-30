ALTER TABLE public.message_store
ADD COLUMN IF NOT EXISTS author_avatar_url text,
ADD COLUMN IF NOT EXISTS author_display_name text;
