ALTER TYPE public.action_type ADD VALUE IF NOT EXISTS 'log';

ALTER TABLE public.automod_rules
ADD COLUMN IF NOT EXISTS log_channel_id bigint;
