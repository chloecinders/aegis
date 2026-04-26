use crate::{
    SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
    transformers::Transformers,
    utils::{cache::trace_cache::TracePoint, is_developer},
};
use aegis_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};
use std::{sync::Arc, time::Duration};

pub struct Trace;

impl Trace {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Command for Trace {
    fn get_name(&self) -> &'static str {
        "trace"
    }
    fn get_short(&self) -> &'static str {
        "Shows command latency and trace breakdown"
    }
    fn get_full(&self) -> &'static str {
        "Shows command latency and trace breakdown of the replied to message."
    }
    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::String("message_id", true)]
    }
    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::string] message_id_arg: Option<String>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if !is_developer(&msg.author) {
            return Ok(());
        }

        let message_id = if let Some(id) = message_id_arg {
            id.parse::<u64>().map_err(|_| CommandError {
                title: String::from("Invalid Message ID"),
                hint: Some(String::from("it must be a numeric ID")),
                arg: None,
            })?
        } else if let Some(referenced_message) = msg.message_reference.clone() {
            referenced_message
                .message_id
                .ok_or_else(|| CommandError {
                    title: String::from("Could not get ID from reply"),
                    hint: None,
                    arg: None,
                })?
                .get()
        } else {
            return Err(CommandError {
                title: String::from(
                    "You must provide a Message ID or reply to a command invocation.",
                ),
                hint: None,
                arg: None,
            });
        };

        trace.point("fetching_trace_from_db");

        let trace_record = sqlx::query!(
            "SELECT command_name, total_duration_nanos, success, error, points FROM command_traces WHERE message_id = $1",
            message_id as i64
        )
        .fetch_optional(&*SQL)
        .await
        .map_err(|err| {
            tracing::error!("DB error fetching trace: {err:?}");
            CommandError {
                title: String::from("Database error while fetching trace"),
                hint: None,
                arg: None,
            }
        })?;

        if let Some(row) = trace_record {
            let points: Vec<TracePoint> = serde_json::from_value(row.points).unwrap_or_default();
            let total_duration = Duration::from_nanos(row.total_duration_nanos as u64);

            let mut description = format!(
                "**{} TRACE**\n-# Status: {} | Total Latency: `{:?}`",
                row.command_name.to_uppercase(),
                if row.success { "Success" } else { "Failed" },
                total_duration
            );

            if let Some(error) = &row.error {
                description.push_str(&format!(" Error: {}", error));
            }

            description.push_str("\n");

            for point in points {
                description.push_str(&format!("{}: `{:?}`\n", point.name, point.duration));
            }

            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(description)
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            let _ = msg.channel_id.send_message(&ctx, reply).await;
        } else {
            return Err(CommandError {
                title: String::from(
                    "No trace found for that message. It may be too old, or wasn't a command.",
                ),
                hint: None,
                arg: None,
            });
        }

        Ok(())
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
