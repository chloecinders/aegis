UPDATE public.guild_settings
SET
    log_channel_ids = (
        SELECT
            jsonb_object_agg (
                CASE
                    WHEN key = 'member_ban' THEN 'member_moderation'
                    WHEN key = 'member_unban' THEN 'member_moderation'
                    WHEN key = 'member_cache' THEN 'member_update'
                    WHEN key = 'member_kick' THEN 'member_moderation'
                    WHEN key = 'member_mute' THEN 'member_moderation'
                    WHEN key = 'member_unmute' THEN 'member_moderation'
                    WHEN key = 'member_warn' THEN 'member_moderation'
                    WHEN key = 'member_softban' THEN 'member_moderation'
                    WHEN key = 'member_update' THEN 'member_update'
                    WHEN key = 'action_update' THEN 'action_update'
                    WHEN key = 'message_delete' THEN 'message_update'
                    WHEN key = 'message_edit' THEN 'message_update'
                    ELSE key
                END,
                value
            )
        FROM
            jsonb_each (log_channel_ids)
    )
WHERE
    log_channel_ids IS NOT NULL;
