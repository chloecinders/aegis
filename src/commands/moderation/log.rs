use std::{collections::HashMap, time::Duration, vec};

use chrono::Utc;
use serenity::{
    all::{
        ButtonStyle, Context, CreateActionRow, CreateAllowedMentions, CreateButton, CreateEmbed,
        CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditMessage,
        Message, Permissions,
    },
    async_trait,
};
use sqlx::{query, query_as};
use tracing::warn;

use crate::{
    SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    database::ActionType,
    event_handler::{CommandError, Handler},
    lexer::Token,
    transformers::Transformers,
};

#[derive(Debug, Clone)]
struct LogRecord {
    id: String,
    r#type: ActionType,
    moderator_id: i64,
    created_at: sqlx::types::chrono::NaiveDateTime,
    updated_at: ::std::option::Option<sqlx::types::chrono::NaiveDateTime>,
    expires_at: ::std::option::Option<sqlx::types::chrono::NaiveDateTime>,
    reason: String,
}

pub struct Log;

impl Log {
    pub fn new() -> Self {
        Self {}
    }

    async fn get_one_response(&self, guild_id: i64, log: String) -> Result<String, CommandError> {
        let res = query!(
            r#"
                SELECT id, type as "type!: ActionType", moderator_id, user_id, created_at, updated_at, active, expires_at, reason FROM actions WHERE guild_id = $1 AND id = $2;
            "#,
            guild_id,
            log
        )
        .fetch_optional(&*SQL).await;

        let data = match res {
            Ok(d) => d,
            Err(err) => {
                warn!("Couldn't fetch log data; err = {err:?}");
                return Err(CommandError {
                    title: String::from("Unable to query the database"),
                    hint: Some(String::from("try again later")),
                    arg: None,
                });
            }
        };

        let Some(data) = data else {
            return Err(CommandError {
                title: String::from("Log not found"),
                hint: Some(String::from("check if you have copied the ID correctly!")),
                arg: None,
            });
        };

        let update_string = if let Some(t) = data.updated_at {
            format!(" | Updated <t:{0}:d> <t:{0}:T>", t.and_utc().timestamp())
        } else {
            String::new()
        };

        let response = if let Some(expiry) = data.expires_at {
            let now = Utc::now().naive_utc();
            let expire_tag = if expiry < now { "Expired" } else { "Expires" };

            format!(
                "**{0}**\n-# Mod: <@{1}> | At: <t:{2}:d> <t:{2}:T>{7} | {3} <t:{4}:d> <t:{4}:T>\n`{5}`\n```\n{6}\n```\n\n",
                data.r#type.to_string().to_uppercase(),
                data.moderator_id,
                data.created_at.and_utc().timestamp(),
                expire_tag,
                expiry.and_utc().timestamp(),
                data.id,
                data.reason.replace("```", "\\`\\`\\`"),
                update_string
            )
        } else {
            format!(
                "**{0}**\n-# Mod: <@{1}> | At <t:{2}:d> <t:{2}:T>{5}\n`{3}`\n```\n{4}\n```\n\n",
                data.r#type.to_string().to_uppercase(),
                data.moderator_id,
                data.created_at.and_utc().timestamp(),
                data.id,
                data.reason,
                update_string
            )
        };

        Ok(response)
    }

    async fn run_one(&self, ctx: Context, msg: Message, log: String) -> Result<(), CommandError> {
        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(
                        self.get_one_response(msg.guild_id.unwrap().get() as i64, log)
                            .await?,
                    )
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            warn!("Could not send message; err = {err:?}");
        }

        Ok(())
    }

    fn create_chunked_response(&self, chunk: &[LogRecord]) -> String {
        let mut response = String::new();

        chunk.iter().for_each(|data| {
            let mut record = data.clone();

            if record.reason.len() > 100 {
                record.reason.truncate(100);
                record.reason.push_str("...");
            }

            let reason =
                if record.reason.chars().all(char::is_whitespace) || record.reason.is_empty() {
                    String::new()
                } else {
                    format!("```\n{}\n```\n", record.reason)
                };

            let update_string = if let Some(t) = record.updated_at {
                format!(" | Updated <t:{0}:d> <t:{0}:T>", t.and_utc().timestamp())
            } else {
                format!(
                    " | At <t:{0}:d> <t:{0}:T>",
                    record.created_at.and_utc().timestamp()
                )
            };

            if let Some(expiry) = record.expires_at {
                let now = Utc::now().naive_utc();
                let expire_tag = if expiry < now { "Expired" } else { "Expires" };

                response.push_str(
                    format!(
                        "**{0}**\n-# Mod: <@{1}>{6} | {2}: <t:{3}:d> <t:{3}:T>\n`{4}`\n{5}\n",
                        record.r#type.to_string().to_uppercase(),
                        record.moderator_id,
                        expire_tag,
                        expiry.and_utc().timestamp(),
                        record.id,
                        reason,
                        update_string
                    )
                    .as_str(),
                );
            } else {
                response.push_str(
                    format!(
                        "**{0}**\n-# Mod: <@{1}>{4}\n`{2}`\n```\n{3}\n```\n",
                        record.r#type.to_string().to_uppercase(),
                        record.moderator_id,
                        record.id,
                        record.reason,
                        update_string
                    )
                    .as_str(),
                );
            }
        });

        response
    }
}

#[async_trait]
impl Command for Log {
    fn get_name(&self) -> &'static str {
        "log"
    }

    fn get_short(&self) -> &'static str {
        "Shows actions taken on a member"
    }

    fn get_full(&self) -> &'static str {
        "Shows the moderation actions taken on a member. This includes warns, bans, kicks, etc."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::Or(
            Box::new(CommandSyntax::User("user", true)),
            Box::new(CommandSyntax::String("id", false)),
        )]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        _handler: &Handler,
        args: Vec<Token>,
        _params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        let mut args_iter = args.clone().into_iter().peekable();
        let Ok(token) = Transformers::user(&ctx, &msg, &mut args_iter).await else {
            match Transformers::string(&ctx, &msg, &mut args.into_iter().peekable()).await {
                Ok(log) => {
                    let Some(CommandArgument::String(id)) = log.contents else {
                        unreachable!()
                    };
                    return self.run_one(ctx, msg, id).await;
                }
                Err(_) => {
                    return Err(CommandError::arg_not_found(
                        "user or id",
                        Some("User || String"),
                    ));
                }
            }
        };

        let Token {
            contents: Some(CommandArgument::User(user)),
            ..
        } = token
        else {
            return Err(CommandError::arg_not_found(
                "user or id",
                Some("User || String"),
            ));
        };

        trace.point("fetching_logs");

        let res = query_as!(
            LogRecord,
            r#"
                SELECT id, type as "type!: ActionType", moderator_id, created_at, updated_at, expires_at, reason FROM actions WHERE user_id = $1 AND guild_id = $2;
            "#,
            user.id.get() as i64,
            msg.guild_id.map(|g| g.get()).unwrap_or(0) as i64
        )
        .fetch_all(&*SQL).await;

        let data = match res {
            Ok(d) => d,
            Err(err) => {
                warn!("Couldn't fetch log data; err = {err:?}");
                return Err(CommandError {
                    title: String::from("Unable to query the database"),
                    hint: Some(String::from("try again later")),
                    arg: None,
                });
            }
        };

        let chunks: Vec<Vec<LogRecord>> = data.chunks(5).map(|c| c.to_vec()).collect();

        let Some(chunk) = chunks.first() else {
            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description("No log entries found.")
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("Could not send message; err = {err:?}");
            }

            return Ok(());
        };

        let mut page_buttons = vec![
            CreateButton::new("first")
                .style(ButtonStyle::Secondary)
                .label("<<")
                .disabled(true),
            CreateButton::new("prev")
                .style(ButtonStyle::Secondary)
                .label("<")
                .disabled(true),
            CreateButton::new("page")
                .style(ButtonStyle::Secondary)
                .label(format!("1/{}", chunks.len()))
                .disabled(true),
            CreateButton::new("next")
                .style(ButtonStyle::Secondary)
                .label(">"),
            CreateButton::new("last")
                .style(ButtonStyle::Secondary)
                .label(">>"),
        ];
        let mut log_buttons = vec![
            CreateButton::new("1")
                .style(ButtonStyle::Secondary)
                .label("1")
                .disabled(chunk.is_empty()),
            CreateButton::new("2")
                .style(ButtonStyle::Secondary)
                .label("2")
                .disabled(chunk.get(1).is_none()),
            CreateButton::new("3")
                .style(ButtonStyle::Secondary)
                .label("3")
                .disabled(chunk.get(2).is_none()),
            CreateButton::new("4")
                .style(ButtonStyle::Secondary)
                .label("4")
                .disabled(chunk.get(3).is_none()),
            CreateButton::new("5")
                .style(ButtonStyle::Secondary)
                .label("5")
                .disabled(chunk.get(4).is_none()),
        ];

        if chunks.len() == 1 {
            page_buttons = page_buttons.into_iter().map(|b| b.disabled(true)).collect();
        }

        let response = self.create_chunked_response(chunk);

        trace.point("sending_chunked_response");

        let reply = CreateMessage::new()
            .add_embed(CreateEmbed::new().description(response).color(BRAND_BLUE))
            .components(vec![
                CreateActionRow::Buttons(page_buttons.clone()),
                CreateActionRow::Buttons(log_buttons.clone()),
            ])
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        let mut new_msg = match msg.channel_id.send_message(&ctx, reply.clone()).await {
            Ok(m) => m,
            Err(err) => {
                warn!("Could not send message; err = {err:?}");
                return Ok(());
            }
        };

        let mut page = 0;

        let get_updated_buttons = |page: usize, disabled: [bool; 4]| -> Vec<CreateButton> {
            let disabled: [bool; 5] = [disabled[0], disabled[1], true, disabled[2], disabled[3]];
            let mut new = page_buttons.clone();

            new = new
                .into_iter()
                .enumerate()
                .map(|(i, mut d)| {
                    if i == 2 {
                        d = d.label(format!("{}/{}", page + 1, chunks.len()));
                    }

                    d.disabled(disabled[i])
                })
                .collect();

            new
        };

        let get_updated_logs = |page: usize| -> Vec<CreateButton> {
            let chunk = chunks.get(page).unwrap();
            let disabled: [bool; 5] = [
                chunk.is_empty(),
                chunk.get(1).is_none(),
                chunk.get(2).is_none(),
                chunk.get(3).is_none(),
                chunk.get(4).is_none(),
            ];
            let mut new = log_buttons.clone();

            new = new
                .into_iter()
                .enumerate()
                .map(|(i, d)| d.disabled(disabled[i]))
                .collect();

            new
        };

        trace.point("awaiting_user_interaction");

        loop {
            let interaction = match new_msg
                .await_component_interaction(&ctx.shard)
                .timeout(Duration::from_secs(60 * 5))
                .await
            {
                Some(i) => i,
                None => {
                    page_buttons = page_buttons.into_iter().map(|b| b.disabled(true)).collect();
                    log_buttons = log_buttons.into_iter().map(|b| b.disabled(true)).collect();
                    let _ = new_msg
                        .edit(
                            &ctx,
                            EditMessage::new().components(vec![
                                CreateActionRow::Buttons(page_buttons),
                                CreateActionRow::Buttons(log_buttons),
                            ]),
                        )
                        .await;
                    return Ok(());
                }
            };

            if interaction.user.id.get() != msg.author.id.get() {
                if let Err(e) = interaction
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
                    warn!("Could not send message; err = {e:?}");
                    return Err(CommandError {
                        title: String::from("Could not send message"),
                        hint: None,
                        arg: None,
                    });
                }

                continue;
            }

            match interaction.data.custom_id.as_str() {
                "first" => {
                    page = 0;
                    let response = self.create_chunked_response(chunks.first().unwrap());
                    if interaction
                        .create_response(
                            &ctx,
                            CreateInteractionResponse::UpdateMessage(
                                CreateInteractionResponseMessage::default()
                                    .add_embed(
                                        CreateEmbed::new().description(response).color(BRAND_BLUE),
                                    )
                                    .components(vec![
                                        CreateActionRow::Buttons(get_updated_buttons(
                                            page,
                                            [true, true, false, false],
                                        )),
                                        CreateActionRow::Buttons(get_updated_logs(page)),
                                    ]),
                            ),
                        )
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                "prev" => {
                    page -= 1;
                    let response = self.create_chunked_response(chunks.get(page).unwrap());
                    let none_prev = if page == 0 {
                        true
                    } else {
                        chunks.get(page - 1).is_none()
                    };
                    if interaction
                        .create_response(
                            &ctx,
                            CreateInteractionResponse::UpdateMessage(
                                CreateInteractionResponseMessage::default()
                                    .add_embed(
                                        CreateEmbed::new().description(response).color(BRAND_BLUE),
                                    )
                                    .components(vec![
                                        CreateActionRow::Buttons(get_updated_buttons(
                                            page,
                                            [none_prev, none_prev, false, false],
                                        )),
                                        CreateActionRow::Buttons(get_updated_logs(page)),
                                    ]),
                            ),
                        )
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                "next" => {
                    page += 1;
                    let response = self.create_chunked_response(chunks.get(page).unwrap());
                    let none_next = chunks.get(page + 1).is_none();
                    if interaction
                        .create_response(
                            &ctx,
                            CreateInteractionResponse::UpdateMessage(
                                CreateInteractionResponseMessage::default()
                                    .add_embed(
                                        CreateEmbed::new().description(response).color(BRAND_BLUE),
                                    )
                                    .components(vec![
                                        CreateActionRow::Buttons(get_updated_buttons(
                                            page,
                                            [false, false, none_next, none_next],
                                        )),
                                        CreateActionRow::Buttons(get_updated_logs(page)),
                                    ]),
                            ),
                        )
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                "last" => {
                    page = chunks.len() - 1;
                    let response = self.create_chunked_response(chunks.last().unwrap());
                    if interaction
                        .create_response(
                            &ctx,
                            CreateInteractionResponse::UpdateMessage(
                                CreateInteractionResponseMessage::default()
                                    .add_embed(
                                        CreateEmbed::new().description(response).color(BRAND_BLUE),
                                    )
                                    .components(vec![
                                        CreateActionRow::Buttons(get_updated_buttons(
                                            page,
                                            [false, false, true, true],
                                        )),
                                        CreateActionRow::Buttons(get_updated_logs(page)),
                                    ]),
                            ),
                        )
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                "1" | "2" | "3" | "4" | "5" => {
                    let log = interaction.data.custom_id.parse::<usize>().unwrap();
                    let id = chunks
                        .get(page)
                        .unwrap()
                        .get(log - 1)
                        .unwrap()
                        .id
                        .to_string();

                    let response = self
                        .get_one_response(interaction.guild_id.unwrap().get() as i64, id)
                        .await?;

                    if interaction
                        .create_response(
                            &ctx,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new().add_embed(
                                    CreateEmbed::new().description(response).color(BRAND_BLUE),
                                ),
                            ),
                        )
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                _ => {}
            };
        }
    }

    fn get_transformers(&self) -> Vec<TransformerFnArc> {
        vec![]
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![
                Permissions::MANAGE_NICKNAMES,
                Permissions::KICK_MEMBERS,
                Permissions::MODERATE_MEMBERS,
                Permissions::BAN_MEMBERS,
            ],
            bot: CommandPermissions::baseline(),
        }
    }
}
