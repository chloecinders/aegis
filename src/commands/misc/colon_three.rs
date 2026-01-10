use serenity::{
    all::{Context, Message},
    async_trait,
};

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::Token,
};
use ouroboros_macros::command;

pub struct ColonThree;

impl ColonThree {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for ColonThree {
    fn get_name(&self) -> &'static str {
        ":3"
    }

    fn get_short(&self) -> &'static str {
        ""
    }

    fn get_full(&self) -> &'static str {
        ":3"
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

    #[command]
    async fn run(&self, ctx: Context, msg: Message) -> Result<(), CommandError> {
        let _ = msg.reply(&ctx, ":3").await;
        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
