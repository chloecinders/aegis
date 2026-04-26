UPDATE public.guild_settings
SET
    log_channel_ids = (
        SELECT
            jsonb_object_agg (
                CASE
                    WHEN key = 'ouroboros_annonucements' THEN 'aegis_announcements'
                    ELSE key
                END,
                value
            )
        FROM
            jsonb_each (log_channel_ids)
    )
WHERE
    log_channel_ids IS NOT NULL;
