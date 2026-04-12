use std::{sync::Arc, time::Duration};

use serenity::{
    all::{
        ActionRowComponent, ButtonStyle, ComponentInteractionDataKind, Context, CreateActionRow,
        CreateAllowedMentions, CreateButton, CreateEmbed, CreateInputText,
        CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, CreateModal,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditMessage,
        InputTextStyle, Message, Permissions,
    },
    async_trait,
};

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
    utils::{
        clamp_chars, consume_pgsql_error, consume_serenity_error,
        rule_cache::{Punishment, Rule},
        tinyid,
    },
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
            CommandSyntax::String("rule", true),
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
        handler: &Handler,
        #[transformers::some_string] name: String,
        #[transformers::some_string] rule: String,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        if name.len() >= 100 {
            return Err(CommandError {
                title: String::from("name argument can only be a max of 100 characters long"),
                hint: None,
                arg: Some(_name_arg),
            });
        }

        if rule.len() >= 500 {
            return Err(CommandError {
                title: String::from("rule argument can only be a max of 500 characters long"),
                hint: None,
                arg: Some(_rule_arg),
            });
        }

        let db_id = tinyid().await;
        let inner = if let Some(stripped) = rule.strip_prefix('/').and_then(|s| s.strip_suffix('/'))
        {
            stripped
        } else {
            &rule
        };
        let is_regex = rule.starts_with('/') && rule.ends_with('/');

        let select_menu = CreateActionRow::SelectMenu(
            CreateSelectMenu::new(
                "punishment_type",
                CreateSelectMenuKind::String {
                    options: vec![
                        CreateSelectMenuOption::new("Warn", "warn"),
                        CreateSelectMenuOption::new("Kick", "kick"),
                        CreateSelectMenuOption::new("Ban", "ban"),
                        CreateSelectMenuOption::new("Softban", "softban"),
                        CreateSelectMenuOption::new("Mute", "mute"),
                        CreateSelectMenuOption::new("Log Only", "log"),
                    ],
                },
            )
            .placeholder("Select a punishment type"),
        );
        let cancel_button = CreateActionRow::Buttons(vec![
            CreateButton::new("cancel")
                .label("Cancel")
                .style(ButtonStyle::Danger),
        ]);

        let components = vec![select_menu.clone(), cancel_button.clone()];
        let disabled_components = vec![
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(
                    "punishment_type",
                    CreateSelectMenuKind::String {
                        options: vec![CreateSelectMenuOption::new("Disabled", "disabled")],
                    },
                )
                .disabled(true),
            ),
            CreateActionRow::Buttons(vec![
                CreateButton::new("cancel")
                    .label("Cancel")
                    .style(ButtonStyle::Danger)
                    .disabled(true),
            ]),
        ];

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**CREATE OCR RULE**\nPlease select the punishment that will be applied when this rule is triggered.",
                    ))
                    .color(BRAND_BLUE),
            )
            .components(components.clone())
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        let mut new_msg = match msg.channel_id.send_message(&ctx, reply).await {
            Ok(m) => m,
            Err(err) => {
                consume_serenity_error(String::from("DLOG RESPONSE"), err);
                return Ok(());
            }
        };

        trace.point("awaiting_user_interaction");
        'outer: loop {
            let interaction = match new_msg
                .await_component_interaction(&ctx.shard)
                .timeout(Duration::from_secs(60 * 5))
                .await
            {
                Some(i) => i,
                None => {
                    let _ = new_msg
                        .edit(&ctx, EditMessage::new().components(disabled_components))
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
                    consume_serenity_error(
                        String::from("CREATE OCR RULE INTERACTION RESPONSE"),
                        err,
                    );
                }

                continue;
            }

            if interaction.data.custom_id.as_str() == "cancel" {
                let _ = new_msg
                    .edit(&ctx, EditMessage::new().components(disabled_components))
                    .await;
                let _ = interaction
                    .create_response(
                        &ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Rule creation cancelled.")
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return Ok(());
            }

            if interaction.data.custom_id.as_str() == "punishment_type" {
                trace.point("processing_punishment_interaction");
                let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind
                else {
                    continue;
                };

                let selected = values.first().cloned().unwrap_or_default();
                if selected.is_empty() {
                    continue;
                }

                let modal = match selected.as_str() {
                    "warn" | "kick" => {
                        CreateModal::new(format!("{}-modal", selected), selected.to_uppercase())
                            .components(vec![
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Paragraph,
                                        "Reason",
                                        "reason",
                                    )
                                    .required(false)
                                    .max_length(500),
                                ),
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Short,
                                        "Silent (y/n)",
                                        "silent",
                                    )
                                    .required(false),
                                ),
                            ])
                    }
                    "ban" | "softban" => {
                        let mut components = vec![
                            CreateActionRow::InputText(
                                CreateInputText::new(InputTextStyle::Paragraph, "Reason", "reason")
                                    .required(false)
                                    .max_length(500),
                            ),
                            CreateActionRow::InputText(
                                CreateInputText::new(
                                    InputTextStyle::Short,
                                    "Clear Days (0-7)",
                                    "days",
                                )
                                .required(false),
                            ),
                            CreateActionRow::InputText(
                                CreateInputText::new(
                                    InputTextStyle::Short,
                                    "Silent (y/n)",
                                    "silent",
                                )
                                .required(false),
                            ),
                        ];
                        if selected == "ban" {
                            components.insert(
                                1,
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Short,
                                        "Duration (seconds)",
                                        "duration",
                                    )
                                    .required(false),
                                ),
                            );
                        }
                        CreateModal::new(format!("{}-modal", selected), selected.to_uppercase())
                            .components(components)
                    }
                    "mute" => {
                        CreateModal::new(format!("{}-modal", selected), selected.to_uppercase())
                            .components(vec![
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Paragraph,
                                        "Reason",
                                        "reason",
                                    )
                                    .required(false)
                                    .max_length(500),
                                ),
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Short,
                                        "Duration (seconds)",
                                        "duration",
                                    )
                                    .required(true),
                                ),
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Short,
                                        "Silent (y/n)",
                                        "silent",
                                    )
                                    .required(false),
                                ),
                            ])
                    }
                    "log" => {
                        CreateModal::new(format!("{}-modal", selected), selected.to_uppercase())
                            .components(vec![
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Paragraph,
                                        "Reason",
                                        "reason",
                                    )
                                    .required(false)
                                    .max_length(500),
                                ),
                                CreateActionRow::InputText(
                                    CreateInputText::new(
                                        InputTextStyle::Short,
                                        "Log Channel ID",
                                        "channel_id",
                                    )
                                    .required(true),
                                ),
                            ])
                    }
                    _ => continue,
                };

                if let Err(err) = interaction
                    .create_response(&ctx, CreateInteractionResponse::Modal(modal))
                    .await
                {
                    consume_serenity_error("CREATE OCR RULE MODAL CREATE".into(), err);
                    break;
                }

                let _ = new_msg
                    .edit(
                        &ctx,
                        EditMessage::new().components(disabled_components.clone()),
                    )
                    .await;

                loop {
                    let interaction = match new_msg
                        .await_modal_interaction(&ctx.shard)
                        .timeout(Duration::from_secs(600))
                        .await
                    {
                        Some(s) => s,
                        None => {
                            break;
                        }
                    };

                    if interaction.data.custom_id != format!("{}-modal", selected) {
                        continue 'outer;
                    }

                    let mut reason = String::from("No reason provided");
                    let mut days = 1u8;
                    let mut silent = false;
                    let mut duration = 0i64;
                    let mut channel_id = 0i64;

                    for component in &interaction.data.components {
                        let Some(component) = component.components.first() else {
                            continue;
                        };

                        match component {
                            ActionRowComponent::InputText(component)
                                if component.custom_id == "reason" =>
                            {
                                reason = component
                                    .value
                                    .clone()
                                    .unwrap_or(String::from("No reason provided"));
                            }
                            ActionRowComponent::InputText(component)
                                if component.custom_id == "days" =>
                            {
                                let str = component.value.clone().unwrap_or_default();
                                let mut iter = crate::lexer::lex(str).into_iter().peekable();
                                if let Ok(token) = Transformers::i32(&ctx, &msg, &mut iter).await {
                                    if let Some(CommandArgument::i32(n)) = token.contents {
                                        days = (n as u8).clamp(0, 7);
                                    }
                                }
                            }
                            ActionRowComponent::InputText(component)
                                if component.custom_id == "silent" =>
                            {
                                let str = component.value.clone().unwrap_or_default();
                                let mut iter = crate::lexer::lex(str).into_iter().peekable();
                                if let Ok(token) = Transformers::bool(&ctx, &msg, &mut iter).await {
                                    if let Some(CommandArgument::bool(b)) = token.contents {
                                        silent = b;
                                    }
                                }
                            }
                            ActionRowComponent::InputText(component)
                                if component.custom_id == "duration" =>
                            {
                                let str = component.value.clone().unwrap_or_default();
                                let mut iter = crate::lexer::lex(str).into_iter().peekable();
                                if let Ok(token) =
                                    Transformers::duration(&ctx, &msg, &mut iter).await
                                {
                                    if let Some(CommandArgument::Duration(d)) = token.contents {
                                        duration = d.num_seconds();
                                    }
                                }
                            }
                            ActionRowComponent::InputText(component)
                                if component.custom_id == "channel_id" =>
                            {
                                let str = component.value.clone().unwrap_or_default();
                                let mut iter = crate::lexer::lex(str).into_iter().peekable();
                                if let Ok(token) =
                                    Transformers::guild_channel(&ctx, &msg, &mut iter).await
                                {
                                    if let Some(CommandArgument::GuildChannel(c)) = token.contents {
                                        channel_id = c.id.get() as i64;
                                    }
                                }
                            }
                            _ => continue,
                        };
                    }

                    trace.point("updating_database");
                    if let Err(err) = sqlx::query(
                        "INSERT INTO automod_rules (id, guild_id, name, type, rule, is_regex, reason, punishment_type, day_clear_amount, duration, silent, log_channel_id) VALUES ($1, $2, $3, 'ocr', $4, $5, $6, CAST($7 AS action_type), $8, $9, $10, $11)"
                    )
                    .bind(db_id.clone())
                    .bind(msg.guild_id.map(|id| id.get()).unwrap_or(1) as i64)
                    .bind(name.clone())
                    .bind(inner)
                    .bind(is_regex)
                    .bind(reason.clone())
                    .bind(selected.clone())
                    .bind(days as i16)
                    .bind(duration)
                    .bind(silent)
                    .bind(channel_id)
                    .execute(&*SQL).await {
                        consume_pgsql_error("OCR RULE CREATE".into(), err);
                        return Err(CommandError {
                            title: String::from("Could not update the database"),
                            hint: None,
                            arg: None,
                        });
                    }

                    {
                        let mut lock = handler.rule_cache.lock().await;
                        let punish = match selected.as_str() {
                            "ban" => Punishment::Ban {
                                reason: reason.clone(),
                                day_clear_amount: days,
                                duration: duration as u64,
                                silent,
                            },
                            "softban" => Punishment::Softban {
                                reason: reason.clone(),
                                day_clear_amount: days,
                                silent,
                            },
                            "kick" => Punishment::Kick {
                                reason: reason.clone(),
                                silent,
                            },
                            "mute" => Punishment::Mute {
                                reason: reason.clone(),
                                duration: duration as u64,
                                silent,
                            },
                            "log" => Punishment::Log {
                                reason: reason.clone(),
                                channel_id: channel_id as u64,
                            },
                            _ => Punishment::Warn {
                                reason: reason.clone(),
                                silent,
                            },
                        };
                        lock.insert_ocr(Rule {
                            name: name.clone(),
                            id: db_id.clone(),
                            rule: inner.to_string(),
                            is_regex,
                            guild_id: msg.guild_id.map(|id| id.get()).unwrap_or(1),
                            punishment: punish,
                        })
                    }

                    let reply = CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .description(format!(
                                    "**CREATED OCR RULE {}**\n-# ID: `{}` | Type: {} | Reason: {}\n```\n{}\n```",
                                    name.to_uppercase(),
                                    db_id,
                                    selected,
                                    clamp_chars(reason, 25),
                                    rule
                                ))
                                .color(BRAND_BLUE)
                        )
                        .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

                    if let Err(err) = interaction
                        .create_response(&ctx, CreateInteractionResponse::Message(reply))
                        .await
                    {
                        consume_serenity_error("SEND CREATE OCR RULE RESPONSE".into(), err);
                        return Ok(());
                    }

                    break;
                }

                break;
            }
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
