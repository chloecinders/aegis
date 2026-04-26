use std::{
    collections::{HashMap, hash_map::Entry::Vacant},
    sync::Arc,
    time::Duration,
};

use serenity::{
    all::{
        ButtonStyle, ComponentInteractionDataKind, Context, CreateActionRow, CreateAllowedMentions,
        CreateButton, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateMessage, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditMessage,
        GuildChannel, Message, Permissions,
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
    lexer::Token,
    transformers::Transformers,
    utils::{LogType, consume_pgsql_error, consume_serenity_error},
};
use aegis_macros::command;

pub struct DefineLog;

impl DefineLog {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for DefineLog {
    fn get_name(&self) -> &'static str {
        "dlog"
    }

    fn get_short(&self) -> &'static str {
        "Defines a channel as a log channel"
    }

    fn get_full(&self) -> &'static str {
        "Defines a channel as a log channel. \
        If no channel is provided the current channel will be selected. \
        You will be able to choose the specific types of events which will get logged within the selected channel."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::Channel("channel", true)]
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
        #[transformers::guild_channel] channel: Option<GuildChannel>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let channel = channel.unwrap_or_else(|| {
            msg.guild(&ctx.cache)
                .unwrap()
                .channels
                .get(&msg.channel_id)
                .unwrap()
                .clone()
        });

        let channel_ids: HashMap<LogType, u64>;

        trace.point("fetching_log_channel_ids");

        {
            let mut lock = GUILD_SETTINGS.lock().await;
            channel_ids = lock
                .get(msg.guild_id.unwrap().get())
                .await
                .unwrap()
                .log
                .log_channel_ids
                .clone();
        }

        let options = LogType::all()
            .into_iter()
            .map(|t| {
                let mut opt = CreateSelectMenuOption::new(t.title(), json::to_string(&t).unwrap());

                if let Some(current_channel) = channel_ids.get(&t) {
                    if *current_channel != channel.id.get() {
                        opt = opt.description(format!(
                            "Currently assigned to <#{current_channel}> - will overwrite!"
                        ))
                    } else {
                        opt = opt.description("Currently assigned to this channel")
                    }
                } else {
                    opt = opt.description(t.description())
                }

                opt
            })
            .collect::<Vec<_>>();
        let options_len = options.len() as u8;

        let components = vec![
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new("type_select", CreateSelectMenuKind::String { options })
                    .max_values(options_len),
            ),
            CreateActionRow::Buttons(vec![
                CreateButton::new("keep")
                    .label("Keep")
                    .style(ButtonStyle::Primary),
                CreateButton::new("all")
                    .label("All")
                    .style(ButtonStyle::Secondary),
                CreateButton::new("reset")
                    .label("Reset")
                    .style(ButtonStyle::Danger),
                CreateButton::new("cancel")
                    .label("Cancel")
                    .style(ButtonStyle::Secondary),
            ]),
        ];

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**DEFINE LOG**\nPlease select the type of events to log in this channel <#{}>.

                        `Keep` - set this channel for events that don’t have a channel yet
                        `All` - set this channel for all events, even if they already have one
                        `Reset` - remove this channel from all events
                        `Cancel` - do nothing",
                        channel.id.get()
                    ))
                    .color(BRAND_BLUE)
            )
            .components(components)
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
                            EditMessage::new().components(vec![
                                CreateActionRow::SelectMenu(
                                    CreateSelectMenu::new(
                                        "type_select",
                                        CreateSelectMenuKind::String { options: vec![] },
                                    )
                                    .max_values(options_len)
                                    .disabled(true),
                                ),
                                CreateActionRow::Buttons(vec![
                                    CreateButton::new("all")
                                        .label("All")
                                        .style(ButtonStyle::Primary)
                                        .disabled(true),
                                    CreateButton::new("reset")
                                        .label("Reset")
                                        .style(ButtonStyle::Danger)
                                        .disabled(true),
                                ]),
                            ]),
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
                    consume_serenity_error(String::from("DLOG INTERACTION RESPONSE"), err);
                    return Err(CommandError {
                        title: String::from("Could not send message"),
                        hint: None,
                        arg: None,
                    });
                }

                continue;
            }

            let new_values = match interaction.data.custom_id.as_str() {
                "type_select" => {
                    let ComponentInteractionDataKind::StringSelect { values } =
                        interaction.data.kind.clone()
                    else {
                        return Err(CommandError {
                            title: String::from("Unexpected interaction data kind"),
                            hint: Some(String::from(
                                "you found an ultra-rare error! please contact the developers about this",
                            )),
                            arg: None,
                        });
                    };

                    let mut current_values = channel_ids.clone();
                    values
                        .into_iter()
                        .map(|t| json::from_str::<LogType>(t).unwrap())
                        .for_each(|t| {
                            current_values.insert(t, channel.id.get());
                        });

                    current_values
                }

                "keep" => {
                    let mut current_values = channel_ids.clone();

                    for log_type in LogType::all() {
                        if let Vacant(e) = current_values.entry(log_type) {
                            e.insert(channel.id.get());
                        }
                    }

                    current_values
                }

                "all" => LogType::all()
                    .into_iter()
                    .map(|t| (t, channel.id.get()))
                    .collect::<HashMap<LogType, u64>>(),

                "reset" => {
                    let mut current_values = channel_ids.clone();

                    for (log_type, channel_id) in current_values.clone() {
                        if channel_id == channel.id.get() {
                            current_values.remove(&log_type);
                        }
                    }

                    current_values
                }

                "cancel" => channel_ids,

                _ => return Ok(()),
            };

            // Not handling any reponse errors from this point since we can't really do anything with the errors anyway
            let _ = new_msg
                .edit(
                    &ctx,
                    EditMessage::new().components(vec![
                        CreateActionRow::SelectMenu(
                            CreateSelectMenu::new(
                                "type_select",
                                CreateSelectMenuKind::String { options: vec![] },
                            )
                            .max_values(options_len)
                            .disabled(true),
                        ),
                        CreateActionRow::Buttons(vec![
                            CreateButton::new("all")
                                .label("All")
                                .style(ButtonStyle::Primary)
                                .disabled(true),
                            CreateButton::new("reset")
                                .label("Reset")
                                .style(ButtonStyle::Danger)
                                .disabled(true),
                        ]),
                    ]),
                )
                .await;

            trace.point("updating_database");

            let res = query!(
                "UPDATE guild_settings SET log_channel_ids = $2 WHERE guild_id = $1",
                msg.guild_id.unwrap().get() as i64,
                json::to_value(&new_values).unwrap()
            )
            .execute(&*SQL)
            .await;

            if let Err(err) = res {
                consume_pgsql_error(String::from("DLOG DB INSERT"), err);
                return Err(CommandError {
                    title: String::from("Could not update the database"),
                    hint: Some(String::from("please try again later")),
                    arg: None,
                });
            }

            {
                let mut lock = GUILD_SETTINGS.lock().await;
                lock.invalidate();
            }

            let _ = interaction
                .create_response(
                    &ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Successfully updated the log channel ids!")
                            .ephemeral(true),
                    ),
                )
                .await;

            let _ = msg.delete(&ctx).await;
            let _ = new_msg.delete(&ctx).await;

            return Ok(());
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
