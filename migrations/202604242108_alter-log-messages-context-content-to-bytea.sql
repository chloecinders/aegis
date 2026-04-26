ALTER TABLE public.log_messages_context
ALTER COLUMN content TYPE bytea USING convert_to (content, 'UTF8');
