ALTER TABLE public.actions
ADD COLUMN IF NOT EXISTS last_reapplied_at timestamp
with
    time zone;
