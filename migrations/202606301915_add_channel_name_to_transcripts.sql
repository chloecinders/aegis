ALTER TABLE public.transcripts
ADD COLUMN IF NOT EXISTS channel_name text;
