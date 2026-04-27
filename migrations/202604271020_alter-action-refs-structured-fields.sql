ALTER TABLE public.action_refs
    ADD COLUMN IF NOT EXISTS ref_message_id  bigint,
    ADD COLUMN IF NOT EXISTS ref_channel_id  bigint,
    ADD COLUMN IF NOT EXISTS ref_author_id   bigint,
    ADD COLUMN IF NOT EXISTS ref_content     bytea;

UPDATE public.action_refs
SET 
    ref_message_id = NULLIF(substring(message_content from '-# ID: `(\d+)`'), '')::bigint,
    ref_channel_id = NULLIF(substring(message_content from '\[Jump\]\(https://discord\.com/channels/@me/(\d+)/\d+\)'), '')::bigint,
    ref_author_id  = NULLIF(substring(message_content from '\| Author: <@(\d+)>'), '')::bigint,
    ref_content    = convert_to(trim(both e'\n' from substring(message_content from '```\n([\s\S]*)\n```')), 'UTF8')
WHERE message_content IS NOT NULL AND message_content LIKE '-# ID:%';

UPDATE public.action_refs
SET ref_content = convert_to(message_content, 'UTF8')
WHERE message_content IS NOT NULL AND message_content NOT LIKE '-# ID:%';

ALTER TABLE public.action_refs
    DROP COLUMN IF EXISTS message_content;
