use std::collections::HashMap;

use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};
use sysinfo::System;
use tracing::warn;

use crate::{
    START_TIME,
    commands::{Command, CommandArgument, CommandCategory, CommandParameter, CommandSyntax},
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token, utils::TraceContext,
};

pub struct About;

impl About {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Command for About {
    fn get_name(&self) -> &'static str {
        "about"
    }

    fn get_short(&self) -> &'static str {
        "Gets general information about the bot"
    }

    fn get_full(&self) -> &'static str {
        "Shows various statistics of the bot. Useful for nerds!"
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
        msg: Message,
        _handler: &Handler,
        _args: Vec<Token>,
        _params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        trace.point("fetching_guild_count");

        let guild_count = ctx.cache.guild_count();

        let uptime = {
            let elapsed = START_TIME.elapsed();
            let seconds = elapsed.as_secs();

            (seconds / 3600, (seconds % 3600) / 60, seconds % 60)
        };

        let memory = {
            trace.point("fetching_sys_memory_usage");

            let mut sys = System::new_all();
            sys.refresh_all();

            sys.process((std::process::id() as usize).into())
                .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
                .unwrap_or(0.0)
        };

        let description = {
            let uptime = if uptime.0 != 0 {
                format!("{}h {}m {}s", uptime.0, uptime.1, uptime.2)
            } else if uptime.1 != 0 {
                format!("{}m {}s", uptime.1, uptime.2)
            } else {
                format!("{}s", uptime.2)
            };

            format!(
                r#"**ABOUT**
Hey, I'm {}!
A moderation bot made for one purpose and one purpose only: Moderation.
I'm currently in private beta but my source code is available at <https://github.com/chloecinders/ouroboros>.
Join our Discord server: <https://discord.gg/SdUf7TrbDq>
Type `+help` to see a list of all commands!

I was made in Rust by chloecinders!

Special thanks to:
```
serenity-rs: Underlying Bot Framework
bupith: Lead Idea Guy
andreashgk: Rust Mentorship
Discord Previews & Rust Central: Bots pre-release testing grounds
```
Nerd Stats:
Version: {}
Servers: {guild_count}
Uptime: {uptime}
Memory: {memory:.2}MB"#,
                ctx.cache.current_user().name,
                env!("CARGO_PKG_VERSION")
            )
        };

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(description)
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(e) = msg.channel_id.send_message(&ctx, reply).await {
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
