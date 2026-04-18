use std::sync::Arc;

use chrono::Utc;
use ouroboros_macros::command;
use serenity::{
    all::{
        Context, CreateAllowedMentions, CreateEmbed, CreateMessage, EditMember, Mentionable,
        Message, Permissions,
    },
    async_trait,
};
use sqlx::query;
use tracing::warn;

use crate::{
    SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    database::ActionType,
    event_handler::{CommandError, Handler},
    lexer::Token,
    transformers::Transformers,
    utils::{LogType, guild_log},
};

pub struct Duration;

impl Duration {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Duration {
    fn get_name(&self) -> &'static str {
        "duration"
    }

    fn get_short(&self) -> &'static str {
        "Modifies the duration of a moderation action"
    }

    fn get_full(&self) -> &'static str {
        "Modifies the duration of a moderation action. \
        Run the log command for the id. \
        The action must be one that accepts a duration, such as ban or mute. \
        The new duration is relative to the time the action has taken place."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::String("id", false),
            CommandSyntax::Duration("duration", true),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::string] arg1: Option<String>,
        #[transformers::string] arg2: Option<String>,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        let mut db_id = None;
        if let Some(reference) = &msg.message_reference {
            let message_id = reference.message_id.unwrap().get();
            if let Ok(Some(record)) = sqlx::query!(
                "SELECT db_id FROM log_messages_context WHERE message_id = $1",
                message_id as i64
            )
            .fetch_optional(&*SQL)
            .await {
                db_id = record.db_id;
            }
        }

        let (id, duration_str) = if let Some(id) = db_id {
            let mut d = String::new();
            if let Some(a1) = arg1 { d.push_str(&a1); }
            if let Some(a2) = arg2 { d.push_str(&a2); }
            (id, d)
        } else {
            let id = arg1.ok_or_else(|| CommandError::arg_not_found("id", Some("provide an ID or reply to a log")))?;
            (id, arg2.unwrap_or_default())
        };

        if duration_str.is_empty() {
             return Err(CommandError::arg_not_found("duration", None));
        }

        let mut fake_args = vec![Token { raw: duration_str, ..Default::default() }].into_iter().peekable();
        let res = Transformers::duration(&ctx, &msg, &mut fake_args).await.map_err(|e| match e {
            crate::commands::TransformerError::CommandError(c) => c,
            crate::commands::TransformerError::MissingArgumentError(_) => CommandError::arg_not_found("duration", None)
        })?;

        let mut duration = chrono::Duration::default();
        if let Some(CommandArgument::Duration(d_val)) = res.contents {
            duration = d_val;
        }
        trace.point("fetching_log_record");

        let res = query!(
            r#"
                SELECT type as "type!: ActionType", user_id, created_at, expires_at, reason FROM actions WHERE guild_id = $1 AND id = $2;
            "#,
            msg.guild_id.map(|g| g.get()).unwrap_or(0) as i64,
            id
        ).fetch_one(&*SQL).await;

        let data = match res {
            Ok(d) => d,
            Err(err) => {
                warn!("Couldn't fetch log data; err = {err:?}");
                return Err(CommandError {
                    title: String::from("Unable to query the database"),
                    hint: Some(String::from("try again later")),
                    arg: None,
                });
            }
        };

        let now = Utc::now().naive_utc();

        if data.expires_at.unwrap_or_default() <= now {
            return Err(CommandError {
                title: String::from("Already expired"),
                hint: Some(String::from("this moderation action has already expired.")),
                arg: None,
            });
        }

        if data.created_at + duration <= now {
            return Err(CommandError {
                title: String::from("This action would set the action duration to the past"),
                hint: Some(String::from(
                    "this would instantly reverse the action. If you are sure this is what you want please use the appropriate command like unban or unmute.",
                )),
                arg: None,
            });
        }

        match data.r#type {
            ActionType::Ban => {
                trace.point("updating_database");

                if let Err(err) = query!(
                    "UPDATE actions SET expires_at = $1 WHERE guild_id = $2 AND id = $3",
                    data.created_at + duration,
                    msg.guild_id.map(|g| g.get()).unwrap_or(0) as i64,
                    id
                )
                .execute(&*SQL)
                .await
                {
                    warn!("Couldn't update duration; err = {err:?}");
                    return Err(CommandError {
                        title: String::from("Unable to query the database"),
                        hint: Some(String::from("try again later")),
                        arg: None,
                    });
                }
            }
            ActionType::Mute => {
                trace.point("updating_mute_duration");

                let time = data.created_at + duration;
                let edit = EditMember::new()
                    .audit_log_reason(&data.reason)
                    .disable_communication_until_datetime(time.and_utc().into());

                let member_result = msg
                    .guild_id
                    .unwrap()
                    .member(&ctx, data.user_id as u64)
                    .await;

                if member_result.is_err() {
                    return Err(CommandError {
                        title: String::from("Unable to update the mute duration"),
                        hint: Some(String::from(
                            "the target of the action isn't in the server anymore. Urge them to join back!",
                        )),
                        arg: None,
                    });
                }

                if let Ok(mut member) = member_result
                    && (member.enable_communication(&ctx).await.is_err()
                        || member
                            .guild_id
                            .edit_member(&ctx, &member, edit)
                            .await
                            .is_err())
                {
                    return Err(CommandError {
                        title: String::from("Unable to update the mute duration"),
                        hint: Some(String::from(
                            "check if the bot has permissions to time the member out",
                        )),
                        arg: None,
                    });
                }

                if let Err(err) = query!(
                    "UPDATE actions SET expires_at = $1 WHERE guild_id = $2 AND id = $3",
                    data.created_at + duration,
                    msg.guild_id.map(|g| g.get()).unwrap_or(0) as i64,
                    id
                )
                .execute(&*SQL)
                .await
                {
                    warn!("Couldn't update duration; err = {err:?}");
                    return Err(CommandError {
                        title: String::from("Unable to query the database"),
                        hint: Some(String::from("try again later")),
                        arg: None,
                    });
                }
            }
            _ => {
                return Err(CommandError {
                    title: String::from("Invalid action type"),
                    hint: Some(String::from(
                        "this moderation action does not have a duration.",
                    )),
                    arg: None,
                });
            }
        };

        let new_expiry_date = data.created_at + duration;

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**`{id}` UPDATED**\n-# New Expiry: {}",
                        new_expiry_date.format("%Y-%m-%d %H:%M:%S")
                    ))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            warn!("Could not send message; err = {err:?}");
        }

        trace.point("submitting_guild_log");

        guild_log(
            &ctx,
            LogType::ActionUpdate,
            msg.guild_id.unwrap(),
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**ACTION UPDATED**\n-# Log ID: `{id}` | Actor: {} | New Expiry: {}",
                        msg.author.mention(),
                        new_expiry_date.format("%Y-%m-%d %H:%M:%S")
                    ))
                    .color(BRAND_BLUE),
            ),
            None,
        )
        .await;

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![
                Permissions::MANAGE_NICKNAMES,
                Permissions::KICK_MEMBERS,
                Permissions::MODERATE_MEMBERS,
                Permissions::BAN_MEMBERS,
            ],
            bot: CommandPermissions::baseline(),
        }
    }
}
