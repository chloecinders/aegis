ALTER TABLE public.log_messages_context
ADD COLUMN IF NOT EXISTS content text;
