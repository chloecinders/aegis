use std::collections::HashMap;

use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};
use tracing::warn;

use crate::{
    commands::{Command, CommandArgument, CommandCategory, CommandParameter, CommandSyntax},
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
};

pub struct ExtractId;

impl ExtractId {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Command for ExtractId {
    fn get_name(&self) -> &'static str {
        "eid"
    }

    fn get_short(&self) -> &'static str {
        "Extracts an id from a message"
    }

    fn get_full(&self) -> &'static str {
        "Checks a replied to message for ids and sends them in separate messages. Useful for people on mobile who don't want to fight with their phone about copying out an id."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Utilities
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
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        let Some(reply) = &msg.referenced_message else {
            return Err(CommandError {
                title: String::from("You must reply to a message to use this command"),
                hint: None,
                arg: None,
            });
        };

        let mut search_text = reply.content.clone();

        trace.point("extracting_ids");
        for embed in &reply.embeds {
            let mut embed_locations = vec![
                embed.title.clone(),
                embed.description.clone(),
                embed.footer.clone().map(|f| f.text),
                embed.author.clone().map(|a| a.name),
            ];
            embed.fields.iter().for_each(|f| {
                embed_locations.push(Some(f.name.clone()));
                embed_locations.push(Some(f.value.clone()));
            });
            embed_locations.into_iter().for_each(|s| {
                search_text.push_str(format!("\n{}", &s.unwrap_or_default()).as_str());
            });
        }

        let ids: Vec<String> = search_text
            .split_whitespace()
            .filter(|part| part.chars().all(|c| c.is_ascii_digit()))
            .filter(|part| part.len() >= 5 && part.len() <= 20)
            .map(|s| s.to_string())
            .collect();

        trace.point("sending_response");
        if ids.is_empty() {
            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description("No IDs found in the referenced message.")
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("Could not send message; err = {err:?}");
            }
        } else {
            let mut iter = ids.into_iter();
            let first_id = iter.next().unwrap();

            let reply = CreateMessage::new()
                .content(first_id)
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("Could not send message; err = {err:?}");
            }

            for id in iter.take(4) {
                let reply = CreateMessage::new().content(id);

                if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                    warn!("Could not send message; err = {err:?}");
                }
            }
        }

        Ok(())
    }
}
