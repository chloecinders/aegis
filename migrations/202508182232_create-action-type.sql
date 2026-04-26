CREATE TYPE public.action_type AS ENUM (
    'warn',
    'ban',
    'kick',
    'softban',
    'timeout',
    'unban',
    'mute',
    'unmute'
);
