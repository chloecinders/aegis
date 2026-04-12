use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
    utils::is_developer,
};
use ouroboros_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};

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
        vec![]
    }
    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        handler: &Handler,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        if !is_developer(&msg.author) {
            return Ok(());
        }

        let Some(referenced_message) = msg.message_reference.clone() else {
            return Err(CommandError {
                title: String::from("You must reply to a command invocation to use this."),
                hint: None,
                arg: None,
            });
        };

        let message_id = referenced_message
            .message_id
            .unwrap_or_else(|| Default::default());

        trace.point("fetching_trace_from_cache");

        let trace_record = {
            let lock = handler.trace_cache.lock().await;
            lock.get(message_id).cloned()
        };

        if let Some(trace_model) = trace_record {
            let mut description = format!(
                "**{} TRACE**\n-# Status: {} | Total Latency: `{:?}`",
                trace_model.command_name.to_uppercase(),
                if trace_model.success {
                    "Success"
                } else {
                    "Failed"
                },
                trace_model.total_duration
            );

            if let Some(error) = &trace_model.error {
                description.push_str(&format!(" Error: {}", error));
            }

            description.push_str("\n");

            for point in trace_model.points {
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
