use sqlx::query;
use tracing::info;

use crate::SQL;

#[derive(Debug, sqlx::Type, Clone)]
#[sqlx(type_name = "action_type", rename_all = "lowercase")]
pub enum ActionType {
    Warn,
    Kick,
    Ban,
    Softban,
    Mute,
    Unban,
    Unmute,
    Log,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Warn => write!(f, "warn"),
            ActionType::Kick => write!(f, "kick"),
            ActionType::Ban => write!(f, "ban"),
            ActionType::Softban => write!(f, "softban"),
            ActionType::Mute => write!(f, "mute"),
            ActionType::Unban => write!(f, "unban"),
            ActionType::Unmute => write!(f, "unmute"),
            ActionType::Log => write!(f, "log"),
        }
    }
}

pub async fn run_migrations() {
    info!("Running database migrations");

    create_action_type_201420250826().await;
    create_actions_223320250818().await;
    create_guild_settings_195120250826().await;
    add_log_bot_to_guild_settings_220420250829().await;
    add_log_mod_to_guild_settings_021020250918().await;
    remove_log_mod_and_change_channel_id_to_jsonb_150020250921().await;
    add_message_cache_store_133120250922().await;
    add_last_reapplied_at_to_actions_160120250923().await;
    migrate_log_types_231320251115().await;
    create_flags_table_205120260105().await;
    create_automod_rules_022920260107().await;
    update_automod_rules_for_log_030420261902().await;
    create_log_messages_context_030420262050().await;
    add_content_to_log_messages_context_041220261300().await;
    create_command_traces_041320261850().await;
    create_action_refs_table_230420261710().await;
}

pub async fn create_actions_223320250818() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.actions
        (
            guild_id bigint NOT NULL,
            user_id bigint NOT NULL,
            reason text COLLATE pg_catalog."default" NOT NULL,
            moderator_id bigint NOT NULL,
            created_at timestamp without time zone NOT NULL DEFAULT now(),
            updated_at timestamp without time zone,
            id character varying(128) COLLATE pg_catalog."default" NOT NULL,
            type action_type NOT NULL DEFAULT 'warn'::action_type,
            active boolean NOT NULL DEFAULT true,
            expires_at timestamp without time zone,
            CONSTRAINT warns_pkey PRIMARY KEY (id)
        )
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!("Couldnt run database migration create_actions_223320250818; Err = {err:?}");
    }
}

pub async fn create_guild_settings_195120250826() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.guild_settings
        (
            guild_id bigint NOT NULL PRIMARY KEY,
            log_channel bigint
        )
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!("Couldnt run database migration create_guild_settings_195120250826; Err = {err:?}");
    }
}

pub async fn create_action_type_201420250826() {
    if let Err(err) = query!(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1
                FROM pg_type t
                JOIN pg_namespace n ON n.oid = t.typnamespace
                WHERE t.typname = 'action_type'
                AND n.nspname = 'public'
            ) THEN
                CREATE TYPE public.action_type AS ENUM
                    ('warn', 'ban', 'kick', 'softban', 'timeout', 'unban', 'mute', 'unmute');
            END IF;
        END$$;
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!("Couldnt run database migration create_guild_settings_195120250826; Err = {err:?}");
    }
}

pub async fn add_log_bot_to_guild_settings_220420250829() {
    if let Err(err) = query!(
        r#"
        ALTER TABLE public.guild_settings
        ADD COLUMN IF NOT EXISTS log_bot BOOLEAN
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration add_log_bot_to_guild_settings_220420250829; Err = {err:?}"
        );
    }
}

pub async fn add_log_mod_to_guild_settings_021020250918() {
    if let Err(err) = query!(
        r#"
        ALTER TABLE public.guild_settings
        ADD COLUMN IF NOT EXISTS log_mod bigint
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration add_log_mod_to_guild_settings_021020250918; Err = {err:?}"
        );
    }
}

pub async fn remove_log_mod_and_change_channel_id_to_jsonb_150020250921() {
    if let Err(err) = query!(
        r#"
        ALTER TABLE public.guild_settings
        DROP COLUMN IF EXISTS log_mod,
        DROP COLUMN IF EXISTS log_channel,
        ADD COLUMN IF NOT EXISTS log_channel_ids jsonb
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration remove_log_mod_and_change_channel_id_to_jsonb_150020250921; Err = {err:?}"
        );
    }
}

pub async fn add_message_cache_store_133120250922() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.message_cache_store
        (
            channel_id bigint NOT NULL,
            message_count integer NOT NULL DEFAULT 0,
            previous_action smallint NOT NULL DEFAULT 0 CHECK (previous_action BETWEEN -1 AND 1),
            PRIMARY KEY (channel_id)
        );
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration add_message_cache_store_133120250922; Err = {err:?}"
        );
    }
}

pub async fn add_last_reapplied_at_to_actions_160120250923() {
    if let Err(err) = query!(
        r#"
        ALTER TABLE public.actions
        ADD COLUMN IF NOT EXISTS last_reapplied_at timestamp with time zone;
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration add_last_reapplied_at_to_actions_160120250923; Err = {err:?}"
        );
    }
}

pub async fn migrate_log_types_231320251115() {
    let r = query!(
        r#"
        UPDATE public.guild_settings
        SET log_channel_ids = (
            SELECT jsonb_object_agg(
                CASE
                    WHEN key='member_ban' THEN 'member_moderation'
                    WHEN key='member_unban' THEN 'member_moderation'
                    WHEN key='member_cache' THEN 'member_update'
                    WHEN key='member_kick' THEN 'member_moderation'
                    WHEN key='member_mute' THEN 'member_moderation'
                    WHEN key='member_unmute' THEN 'member_moderation'
                    WHEN key='member_warn' THEN 'member_moderation'
                    WHEN key='member_softban' THEN 'member_moderation'
                    WHEN key='member_update' THEN 'member_update'
                    WHEN key='action_update' THEN 'action_update'
                    WHEN key='message_delete' THEN 'message_update'
                    WHEN key='message_edit' THEN 'message_update'
                    ELSE key
                END,
                value
            )
            FROM jsonb_each(log_channel_ids)
        )
        WHERE log_channel_ids IS NOT NULL;
        "#
    )
    .execute(&*SQL)
    .await;

    if let Err(err) = r {
        panic!("Couldnt run database migration migrate_log_types_231320251115; Err = {err:?}");
    }
}

pub async fn create_flags_table_205120260105() {
    let r = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.user_flags (
            user_id TEXT NOT NULL,
            flag TEXT NOT NULL,
            value BIGINT NOT NULL,
            expires_at TIMESTAMPTZ,

            PRIMARY KEY (user_id, flag)
        );
        "#
    )
    .execute(&*SQL)
    .await;

    if let Err(err) = r {
        panic!("Couldnt run database migration create_flags_table_205120260105; Err = {err:?}");
    }
}

pub async fn create_automod_rules_022920260107() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.automod_rules
        (
            id character varying(128) COLLATE pg_catalog."default" NOT NULL,
            guild_id bigint NOT NULL,
            name character varying(128) COLLATE pg_catalog."default" NOT NULL,
            type character varying(128) COLLATE pg_catalog."default" NOT NULL,
            rule character varying(512) COLLATE pg_catalog."default" NOT NULL,
            is_regex boolean NOT NULL,
            created_at timestamp without time zone NOT NULL DEFAULT now(),
            reason text COLLATE pg_catalog."default" NOT NULL,
            punishment_type action_type NOT NULL DEFAULT 'warn'::action_type,
            duration BIGINT,
            day_clear_amount SMALLINT,
            silent BOOL,
            CONSTRAINT automod_rules_pkey PRIMARY KEY (id)
        )
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!("Couldnt run database migration create_automod_rules_022920260107; Err = {err:?}");
    }
}

pub async fn update_automod_rules_for_log_030420261902() {
    if let Err(err) = sqlx::query(
        r#"
        ALTER TYPE public.action_type ADD VALUE IF NOT EXISTS 'log';
        "#,
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration update_automod_rules_for_log_030420261902; Err = {err:?}"
        );
    }

    if let Err(err) = query!(
        r#"
        ALTER TABLE public.automod_rules
        ADD COLUMN IF NOT EXISTS log_channel_id bigint;
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration update_automod_rules_for_log_030420261902; Err = {err:?}"
        );
    }
}

pub async fn create_log_messages_context_030420262050() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.log_messages_context
        (
            message_id bigint NOT NULL PRIMARY KEY,
            guild_id bigint NOT NULL,
            target_id bigint NOT NULL,
            moderator_id bigint NOT NULL,
            db_id character varying(128) COLLATE pg_catalog."default",
            content text
        );
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration create_log_messages_context_030420262050; Err = {err:?}"
        );
    }
}

pub async fn add_content_to_log_messages_context_041220261300() {
    if let Err(err) = query!(
        r#"
        ALTER TABLE public.log_messages_context
        ADD COLUMN IF NOT EXISTS content text;
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration add_content_to_log_messages_context_041220261300; Err = {err:?}"
        );
    }
}

pub async fn create_command_traces_041320261850() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.command_traces
        (
            message_id bigint NOT NULL PRIMARY KEY,
            command_name text NOT NULL,
            total_duration_nanos bigint NOT NULL,
            success boolean NOT NULL,
            error text,
            points jsonb NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now()
        );
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!("Couldnt run database migration create_command_traces_041320261850; Err = {err:?}");
    }
}

pub async fn create_action_refs_table_230420261710() {
    if let Err(err) = query!(
        r#"
        CREATE TABLE IF NOT EXISTS public.action_refs
        (
            action_id character varying(128) COLLATE pg_catalog."default" NOT NULL,
            message_content text,
            image_url text,
            CONSTRAINT action_refs_pkey PRIMARY KEY (action_id)
        );
        "#
    )
    .execute(&*SQL)
    .await
    {
        panic!(
            "Couldnt run database migration create_action_refs_table_230420261710; Err = {err:?}"
        );
    }
}
