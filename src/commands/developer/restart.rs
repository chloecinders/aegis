use serenity::{
    all::{Context, Message},
    async_trait,
};

use crate::{
    ShardManagerContainer,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::Token,
    utils::is_developer,
};
use aegis_macros::command;

pub struct Restart;

impl Restart {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Command for Restart {
    fn get_name(&self) -> &'static str {
        "restart"
    }
    fn get_short(&self) -> &'static str {
        "Restarts the bot"
    }
    fn get_full(&self) -> &'static str {
        "Restarts the bot"
    }
    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }
    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    #[command]
    async fn run(&self, ctx: Context, msg: Message) -> Result<(), CommandError> {
        if !is_developer(&msg.author) {
            return Ok(());
        }

        let _ = msg.reply(&ctx, "Restarting...").await;

        let data = ctx.data.read().await;
        let shard_manager = data.get::<ShardManagerContainer>().unwrap();
        shard_manager.shutdown_all().await;

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
