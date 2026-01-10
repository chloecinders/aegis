use core::arch;
use std::{
    collections::{HashMap, hash_map::Entry::Vacant},
    sync::{Arc, OnceLock},
    time::Duration,
};

use serenity::{
    all::{
        ActionRowComponent, ButtonStyle, ComponentInteraction, ComponentInteractionDataKind, Context, CreateActionRow, CreateAllowedMentions, CreateButton, CreateEmbed, CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, CreateModal, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditMessage, Guild, GuildChannel, InputTextStyle, Interaction, Message, Permissions
    },
    async_trait, json,
};
use sqlx::query;

use crate::{
    GUILD_SETTINGS, SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::{Token, lex},
    transformers::{self, Transformers},
    utils::{LogType, clamp_chars, consume_pgsql_error, consume_serenity_error, rule_cache::{Punishment, Rule}, tinyid},
};
use ouroboros_macros::command;

pub struct CreateOcrRule;

impl CreateOcrRule {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for CreateOcrRule {
    fn get_name(&self) -> &'static str {
        "create_ocr_rule"
    }

    fn get_short(&self) -> &'static str {
        "Creates a new automoderation rule with OCR"
    }

    fn get_full(&self) -> &'static str {
        "Creates a new automoderation rule with OCR. \
        The bot will then automatically scan images for the selected strings and take actions automatically. \
        Using slashes at the start and end of a rule will be interpreted as Regex using the Rust Regex crate. \
        Otherwise simple string matching is used (case insensitive). \
        Note that OCR is still fairly inaccurate and can cause false positives, use with caution. \
        The `ocr_check` command runs an image through OCR. This will output a string with those potential inaccuracies. \
        Use that string for determining rules."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::String("name", true),
            CommandSyntax::String("rule", true)
        ]
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
        #[transformers::some_string] name: String,
        #[transformers::some_string] rule: String
    ) -> Result<(), CommandError> {
        if name.len() >= 100 {
            return Err(CommandError { title: String::from("name argument can only be a max of 100 characters long"), hint: None, arg: Some(_name_arg) })
        }

        if rule.len() >= 500 {
            return Err(CommandError { title: String::from("rule argument can only be a max of 500 characters long"), hint: None, arg: Some(_rule_arg) })
        }

        let db_id = tinyid().await;
        let inner = if let Some(stripped) = rule.strip_prefix('/').and_then(|s| s.strip_suffix('/')) { stripped } else { &rule };
        let is_regex = rule.starts_with('/') && rule.ends_with('/');

        let buttons = vec![CreateActionRow::Buttons(vec![
            CreateButton::new("softban").label("Softban"),
        ])];
        let disabled_buttons = vec![CreateActionRow::Buttons(vec![
            CreateButton::new("softban").label("Softban").disabled(true),
        ])];

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**CREATE OCR RULE**\nPlease select the punishment that will be applied when this rule is triggered.",
                    ))
                    .color(BRAND_BLUE)
            )
            .components(buttons.clone())
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        let mut new_msg = match msg.channel_id.send_message(&ctx, reply).await {
            Ok(m) => m,
            Err(err) => {
                consume_serenity_error(String::from("DLOG RESPONSE"), err);
                return Ok(());
            }
        };

        loop {
            let interaction = match new_msg
                .await_component_interaction(&ctx.shard)
                .timeout(Duration::from_secs(60 * 5))
                .await
            {
                Some(i) => i,
                None => {
                    let _ = new_msg
                        .edit(
                            &ctx,
                            EditMessage::new().components(disabled_buttons),
                        )
                        .await;
                    return Ok(());
                }
            };

            if interaction.user.id != msg.author.id {
                if let Err(err) = interaction
                    .create_response(
                        &ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("You are not the author of the original message!")
                                .ephemeral(true),
                        ),
                    )
                    .await
                {
                    consume_serenity_error(String::from("CREATE OCR RULE INTERACTION RESPONSE"), err);
                    return Err(CommandError {
                        title: String::from("Could not send message"),
                        hint: None,
                        arg: None,
                    });
                }

                continue;
            }

            match interaction {
                interaction if interaction.data.custom_id.as_str() == "softban" => {
                    let modal = CreateModal::new("softban-modal", "Softban")
                        .components(vec![
                            CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph, "Reason", "reason").required(false).max_length(500)),
                            CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Clear Days (0-7)", "days").required(false)),
                            CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Silent (y/n)", "silent").required(false))
                        ]);

                    if let Err(err) = interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await {
                        consume_serenity_error("CREATE OCR RULE MODAL CREATE".into(), err);
                        break;
                    }

                    let interaction = match new_msg.await_modal_interaction(&ctx.shard).timeout(Duration::from_secs(600)).await {
                        Some(s) => s,
                        None => {
                            break;
                        },
                    };

                    let mut reason = String::from("No reason provided");
                    let mut days = 1u8;
                    let mut silent = false;

                    for component in &interaction.data.components {
                        let Some(component) = component.components.first() else { continue };

                        match component {
                            ActionRowComponent::InputText(component) if component.custom_id == "reason" => {
                                reason = component.value.clone().unwrap_or(String::from("No reason provided"));
                            },
                            ActionRowComponent::InputText(component) if component.custom_id == "days" => {
                                let str = component.value.clone().unwrap_or_default();
                                days = str.parse::<u8>().unwrap_or(0).clamp(0, 7);
                            },
                            ActionRowComponent::InputText(component) if component.custom_id == "silent" => {
                                let str = component.value.clone().unwrap_or_default();
                                silent = matches!(
                                    str.to_lowercase().as_str(),
                                    "true"
                                        | "y"
                                        | "yes"
                                        | "yeah"
                                        | "t"
                                        | "ok"
                                        | "on"
                                        | "enabled"
                                        | "1"
                                        | "enable"
                                        | "check"
                                        | "checked"
                                        | "sure"
                                        | "yep"
                                        | "aye"
                                        | "valid"
                                        | "correct"
                                );
                            }
                            _ => { continue }
                        };
                    }

                    if let Err(err) = query!(r#"
                        INSERT INTO automod_rules (id, guild_id, name, type, rule, is_regex, reason, punishment_type, day_clear_amount, silent)
                        VALUES ($1, $2, $3, 'ocr', $4, $5, $6, 'softban', $7, $8)
                    "#, db_id, msg.guild_id.map(|id| id.get()).unwrap_or(1) as i64, name, inner, is_regex, reason, days as i16, silent)
                    .execute(&*SQL).await {
                        consume_pgsql_error("OCR SOFTBAN RULE CREATE".into(), err);
                        return Err(CommandError {
                            title: String::from("Could not update the database"),
                            hint: None,
                            arg: None,
                        });
                    }

                    {
                        let mut lock = _handler.rule_cache.lock().await;
                        lock.insert_ocr(Rule {
                            name: name.clone(),
                            id: db_id.clone(),
                            rule: inner.to_string(),
                            is_regex,
                            guild_id: msg.guild_id.map(|id| id.get()).unwrap_or(1),
                            punishment: Punishment::Softban { reason: reason.clone(), day_clear_amount: days, silent }
                        })
                    }

                    let reply = CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .description(format!(
                                    "**CREATED OCR RULE {}**\n-# ID: `{}` | Type: softban | Reason: {} | Days to clear: {} | Silent: {}\n```\n{}\n```",
                                    name.to_uppercase(),
                                    db_id,
                                    clamp_chars(reason, 25),
                                    days,
                                    silent,
                                    rule
                                ))
                                .color(BRAND_BLUE)
                        )
                        .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

                    if let Err(err) = interaction.create_response(&ctx, CreateInteractionResponse::Message(reply)).await {
                        consume_serenity_error("SEND CREATE OCR RULE RESPONSE".into(), err);
                        return Ok(());
                    }

                    break;
                },
                _ => return Ok(()),
            };
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
