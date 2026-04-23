use std::sync::Arc;

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
    utils::{consume_pgsql_error, consume_serenity_error},
};
use ouroboros_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message, Permissions},
    async_trait,
};
use sqlx::Row;

pub struct DeleteRule;

impl DeleteRule {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for DeleteRule {
    fn get_name(&self) -> &'static str {
        "delete_rule"
    }

    fn get_short(&self) -> &'static str {
        "Deletes an existing automoderation rule"
    }

    fn get_full(&self) -> &'static str {
        "Deletes an existing automoderation rule. Run the rules command to list all existing rules and their IDs."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::String("id", true)]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Admin
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
        #[transformers::some_string] id: String,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let Some(guild_id) = msg.guild_id else {
            return Err(CommandError {
                title: String::from("Unexpected error has occurred."),
                hint: Some(String::from("could not get guild id")),
                arg: None,
            });
        };

        trace.point("fetching_rule");
        let row =
            sqlx::query("SELECT id, name, type FROM automod_rules WHERE id = $1 AND guild_id = $2")
                .bind(id.as_str())
                .bind(guild_id.get() as i64)
                .fetch_optional(&*SQL)
                .await;

        let row = match row {
            Ok(Some(r)) => r,
            Ok(None) => {
                return Err(CommandError {
                    title: format!("No rule with ID `{id}` found in this server"),
                    hint: Some(String::from("use +rules to list all rules")),
                    arg: Some(_id_arg),
                });
            }
            Err(err) => {
                consume_pgsql_error("DELETE RULE FETCH".into(), err);
                return Err(CommandError {
                    title: String::from("Could not query the database"),
                    hint: Some(String::from("please try again later")),
                    arg: None,
                });
            }
        };

        let rule_name: String = row.get("name");
        let rule_type: String = row.get("type");

        trace.point("updating_database");
        if let Err(err) = sqlx::query("DELETE FROM automod_rules WHERE id = $1")
            .bind(id.as_str())
            .execute(&*SQL)
            .await
        {
            consume_pgsql_error("DELETE RULE".into(), err);
            return Err(CommandError {
                title: String::from("Could not delete the rule"),
                hint: Some(String::from("please try again later")),
                arg: None,
            });
        }

        {
            let mut lock = handler.rule_cache.lock().await;
            lock.remove(&id);
        }

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**DELETED {} RULE {}**\n-# ID: `{}`",
                        rule_type.to_uppercase(),
                        rule_name.to_uppercase(),
                        id,
                    ))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            consume_serenity_error("DELETE RULE RESPONSE".into(), err);
        }

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::ADMINISTRATOR],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
