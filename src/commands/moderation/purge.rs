use std::{collections::HashMap, sync::Arc};

use chrono::{Duration, Utc};
use serenity::{
    all::{Context, CreateEmbed, CreateMessage, GetMessages, Mentionable, Message, Permissions},
    async_trait,
};
use tracing::warn;

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::{Token, lex},
    transformers::Transformers,
    utils::{LogType, guild_log},
};
use aegis_macros::command;

pub struct Purge;

impl Purge {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Purge {
    fn get_name(&self) -> &'static str {
        "purge"
    }

    fn get_short(&self) -> &'static str {
        "Mass deletes a specific amount of messages"
    }

    fn get_full(&self) -> &'static str {
        "Mass deletes a specific amount of messages from a channel. \
        Messages older than 2 weeks are ignored. \
        Count must be between 2 and 99. \
        Optional filters can be applied after the count: \
        \n`+user/+u @aegis` -> Message Author \
        \n`+string/+s \"content\"` -> Message Content"
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::Number("count", true), CommandSyntax::Filters]
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
        #[transformers::i32] count: i32,
        #[transformers::consume] filters: String,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if !(2..=99).contains(&count) {
            return Err(CommandError {
                title: String::from("Message count must be between 2 and 99"),
                hint: None,
                arg: Some(_count_arg.clone()),
            });
        }

        let mut lex = lex(filters).into_iter().peekable();
        let mut filters: HashMap<&str, CommandArgument> = HashMap::new();

        while let Some(token) = lex.next() {
            match token.raw.as_str() {
                "+u" | "+user" => {
                    if let Ok(Token {
                        contents: Some(cmd_arg),
                        ..
                    }) = Transformers::user(&ctx, &msg, &mut lex).await
                    {
                        filters.insert("user", cmd_arg);
                    }
                }

                "+s" | "+string" => {
                    filters.insert(
                        "string",
                        CommandArgument::String(lex.next().map(|t| t.raw).unwrap_or_default()),
                    );
                }

                _ => {}
            }
        }

        trace.point("fetching_channel_messages");

        let mut messages = match msg
            .channel_id
            .messages(&ctx, GetMessages::new().limit(100))
            .await
        {
            Ok(m) => m,
            Err(err) => {
                warn!("Got error while fetching messages; err = {err:?}");
                return Err(CommandError {
                    title: String::from("Could not get channel messages"),
                    hint: Some(String::from(
                        "there is currently a bug where this command fails if the last 100 messages in the channel have messages with components in them. Will be fixed soon sorry!",
                    )),
                    arg: None,
                });
            }
        };

        let now = Utc::now();
        let two_weeks = Duration::weeks(2);

        messages.remove(0);

        trace.point("applying_filters");

        let mut filtered = messages
            .iter()
            .filter(|m| {
                let diff = (now - *m.timestamp).num_seconds().abs();

                if diff <= two_weeks.num_seconds() {
                    return true;
                }

                false
            })
            .collect::<Vec<_>>();

        if let Some(CommandArgument::User(user)) = filters.get("user") {
            filtered = filtered
                .into_iter()
                .filter(|m| {
                    if m.author.id.get() == user.id.get() {
                        return true;
                    }

                    false
                })
                .collect::<Vec<_>>();
        }

        if let Some(CommandArgument::String(content)) = filters.get("string") {
            filtered = filtered
                .into_iter()
                .filter(|m| {
                    if m.content.contains(content) {
                        return true;
                    }

                    false
                })
                .collect::<Vec<_>>();
        }

        let ids = filtered
            .clone()
            .into_iter()
            .map(|m| m.id.get().to_string())
            .collect::<Vec<_>>();

        let final_count = ids.len();

        trace.point("executing_sanctions");

        if msg
            .channel_id
            .delete_messages(&ctx, filtered)
            .await
            .is_err()
        {
            return Err(CommandError {
                title: String::from("Could not delete channel messages"),
                hint: Some(String::from(
                    "make sure the bot has enough permissions to delete the messages of this channel",
                )),
                arg: None,
            });
        };

        trace.point("submitting_guild_log");

        guild_log(
            &ctx,
            LogType::MemberModeration,
            msg.guild_id.unwrap(),
            CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(format!(
                            "**MESSAGES PURGED**\n-# Actor: {} | Channel: <#{}> | Count: {}\n```\n{}\n```",
                            msg.author.mention(),
                            msg.channel_id.get(),
                            final_count,
                            ids.join("\n")
                        ))
                        .color(BRAND_BLUE)
                ),
            None,
        ).await;

        let _ = msg.delete(&ctx).await;

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::MANAGE_MESSAGES],
            one_of: vec![],
            bot: [
                CommandPermissions::baseline().as_slice(),
                CommandPermissions::moderation().as_slice(),
            ]
            .concat(),
        }
    }
}
