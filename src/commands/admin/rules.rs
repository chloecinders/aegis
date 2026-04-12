use std::{sync::Arc, time::Duration, vec};

use chrono::TimeDelta;
use ouroboros_macros::command;
use serenity::{
    all::{
        ButtonStyle, Context, CreateActionRow, CreateAllowedMentions, CreateButton, CreateEmbed,
        CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditMessage,
        Message, Permissions,
    },
    async_trait,
};
use sqlx::query_as;
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
    guild_id: i64,
    name: String,
    r#type: String,
    rule: String,
    is_regex: bool,
    created_at: sqlx::types::chrono::NaiveDateTime,
    reason: String,
    punishment_type: ActionType,
    duration: ::std::option::Option<i64>,
    day_clear_amount: ::std::option::Option<i16>,
    silent: ::std::option::Option<bool>,
}

pub struct Rules;

impl Rules {
    pub fn new() -> Self {
        Self {}
    }

    async fn get_one_response(&self, guild_id: i64, log: String) -> Result<String, CommandError> {
        let res = query_as!(
            LogRecord,
            r#"
                SELECT id, guild_id, name, type, rule, is_regex, created_at, reason, punishment_type as "punishment_type!: ActionType", duration, day_clear_amount, silent FROM automod_rules WHERE guild_id = $1 AND id = $2;
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

        let response = self.create_chunked_response(&[data], false);

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

    fn create_chunked_response(&self, chunk: &[LogRecord], compact: bool) -> String {
        let mut response = String::new();

        chunk.iter().for_each(|data| {
            let record = data.clone();

            let rule = if record.rule.len() > 100 && compact {
                format!("{}...", &record.rule[..97])
            } else {
                record.rule
            };

            let rule = if record.is_regex {
                format!("/{}/", rule)
            } else {
                rule
            };

            let time_string = if let Some(duration) = record.duration.map(|d| TimeDelta::seconds(d))
                && !duration.is_zero()
            {
                let (time, mut unit) = match () {
                    _ if (duration.num_days() as f64 / 365.0).fract() == 0.0
                        && duration.num_days() >= 365 =>
                    {
                        (duration.num_days() / 365, String::from("year"))
                    }
                    _ if (duration.num_days() as f64 / 30.0).fract() == 0.0
                        && duration.num_days() >= 30 =>
                    {
                        (duration.num_days() / 30, String::from("month"))
                    }
                    _ if duration.num_days() != 0 => (duration.num_days(), String::from("day")),
                    _ if duration.num_hours() != 0 => (duration.num_hours(), String::from("hour")),
                    _ if duration.num_minutes() != 0 => {
                        (duration.num_minutes(), String::from("minute"))
                    }
                    _ if duration.num_seconds() != 0 => {
                        (duration.num_seconds(), String::from("second"))
                    }
                    _ => (0, String::new()),
                };

                if time > 1 {
                    unit += "s";
                }

                format!("for {time} {unit}")
            } else {
                String::from("permanent")
            };

            let punishment = match record.punishment_type {
                ActionType::Warn => format!(" | Punishment: Warn | Reason: {}", record.reason),
                ActionType::Softban => {
                    format!(" | Punishment: Softban | Reason: {}", record.reason)
                }
                ActionType::Ban => format!(
                    " | Punishment: Ban | Reason: {} | Duration: {}",
                    record.reason, time_string
                ),
                _ => String::from("unexpected"),
            };

            response.push_str(
                format!(
                    "**{0}**\n-# ID: `{1}` | Type: {2} | Created: <t:{3}:d> <t:{3}:T>{4}\n```\n{5}\n```\n\n",
                    record.name,
                    record.id,
                    record.r#type,
                    record.created_at.and_utc().timestamp(),
                    punishment,
                    rule
                )
                .as_str(),
            );
        });

        response
    }
}

#[async_trait]
impl Command for Rules {
    fn get_name(&self) -> &'static str {
        "rules"
    }

    fn get_short(&self) -> &'static str {
        "Lists all bot moderation rules"
    }

    fn get_full(&self) -> &'static str {
        "Lists all bot moderations rules, such as OCR rules."
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
        #[transformers::string] id: Option<String>,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        if let Some(id) = id {
            return self.run_one(ctx, msg, id).await;
        }

        trace.point("fetching_rules");

        let res = query_as!(
            LogRecord,
            r#"
                SELECT id, guild_id, name, type, rule, is_regex, created_at, reason, punishment_type as "punishment_type!: ActionType", duration, day_clear_amount, silent FROM automod_rules WHERE guild_id = $1;
            "#,
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

        let response = self.create_chunked_response(chunk, true);

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
                    let response = self.create_chunked_response(chunks.first().unwrap(), true);
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
                    let response = self.create_chunked_response(chunks.get(page).unwrap(), true);
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
                    let response = self.create_chunked_response(chunks.get(page).unwrap(), true);
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
                    let response = self.create_chunked_response(chunks.last().unwrap(), true);
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

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::ADMINISTRATOR],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
