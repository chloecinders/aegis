ALTER TABLE public.guild_settings
DROP COLUMN IF EXISTS log_mod,
DROP COLUMN IF EXISTS log_channel,
ADD COLUMN IF NOT EXISTS log_channel_ids jsonb
