use ouroboros_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};
use tracing::warn;

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandSyntax,
        TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
};

pub struct CacheSize;

impl CacheSize {
    pub fn new() -> Self {
        Self
    }

    fn format_bytes(bytes: usize) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }
}

#[async_trait]
impl Command for CacheSize {
    fn get_name(&self) -> &'static str {
        "cachesize"
    }

    fn get_short(&self) -> &'static str {
        "View cache memory usage"
    }

    fn get_full(&self) -> &'static str {
        "Shows an approximate memory footprint for all internal caches."
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        handler: &Handler,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        if !crate::utils::is_developer(&msg.author) {
            return Ok(());
        }

        let msg_size = handler.message_cache.lock().await.byte_footprint();
        let perm_size = handler.permission_cache.lock().await.byte_footprint().await;
        let rule_size = handler.rule_cache.lock().await.byte_footprint();

        let total = msg_size + perm_size + rule_size;

        trace.point("sending_response");
        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**INTERNAL CACHE FOOTPRINT**\n\
                        Message Cache: `{}`\n\
                        Permission Cache: `{}`\n\
                        Rule Cache: `{}`\n\n\
                        Total: `{}`",
                        Self::format_bytes(msg_size),
                        Self::format_bytes(perm_size),
                        Self::format_bytes(rule_size),
                        Self::format_bytes(total)
                    ))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            warn!("Could not send message; err = {err:?}");
        }

        Ok(())
    }
}
