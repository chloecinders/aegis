use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message as DiscordMessage},
    async_trait,
};
use tracing::warn;

use crate::{
    ShardManagerContainer,
    commands::{Command, CommandArgument, CommandCategory, CommandParameter, CommandSyntax},
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
};

pub struct Ping;

impl Ping {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Ping {
    fn get_name(&self) -> &'static str {
        "ping"
    }

    fn get_short(&self) -> &'static str {
        "Gets the bots current latency"
    }

    fn get_full(&self) -> &'static str {
        "Gets the bots HTTP and gateway latency. Useful for checking if the bot is lagging."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Misc
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    async fn run(
        &self,
        ctx: Context,
        msg: DiscordMessage,
        _handler: &Handler,
        _args: Vec<Token>,
        _params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        // the time it takes to request the current user object and get it back
        let http = {
            trace.point("testing_http_latency");
            let start = Instant::now();
            let _ = ctx.http.get_current_user().await;
            start.elapsed().as_millis()
        };

        // the websocket gateway latency, may take a bit to produce an actual number after restart due to the weird way serenity measures latency
        let gateway = {
            trace.point("testing_gateway_latency");
            let data_read = ctx.data.read().await;
            let shard_manager = data_read.get::<ShardManagerContainer>().unwrap().clone();
            let runners = shard_manager.runners.lock().await;
            let shard_info = runners.get(&ctx.shard_id).unwrap();
            shard_info
                .latency
                .unwrap_or(Duration::default())
                .as_millis()
        };

        let message = CreateMessage::new()
            .embed(
                CreateEmbed::new()
                    .description(format!("**PING**\nHTTP: {http}ms\nGateway: {gateway}ms",))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(e) = msg.channel_id.send_message(&ctx, message).await {
            warn!("Could not send message; err = {e:?}");
            return Err(CommandError {
                title: String::from("Could not send message"),
                hint: None,
                arg: None,
            });
        }

        Ok(())
    }
}
