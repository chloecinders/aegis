use serenity::{
    all::{Context, CreateMessage, Message},
    async_trait,
};

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandSyntax,
        TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::Token,
    utils::is_developer,
};
use ouroboros_macros::command;

pub struct ScheduleDowntime;

impl ScheduleDowntime {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for ScheduleDowntime {
    fn get_name(&self) -> &'static str {
        "sd"
    }

    fn get_short(&self) -> &'static str {
        ""
    }

    fn get_full(&self) -> &'static str {
        "Placeholder"
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(&self, ctx: Context, msg: Message) -> Result<(), CommandError> {
        if is_developer(&msg.author) {
            // coming soon
            let _ = msg
                .channel_id
                .send_message(&ctx, CreateMessage::new().content("fuck you"))
                .await;
        }

        Ok(())
    }
}
