#![allow(dead_code)]
use std::{collections::HashMap, sync::Arc};

use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message, Permissions},
    async_trait,
    json::{self, Value},
};
use tracing::{error, warn};

use crate::{
    GUILD_SETTINGS, SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerError, TransformerFn, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::CommandError,
    lexer::Token,
    transformers::Transformers,
    utils::Settings,
};

/// The signature of a Transformer function boxed
type BoxedTransformerFn = Box<TransformerFn>;

// Disabled until its actually needed
pub struct Config;

impl Config {
    pub fn new() -> Self {
        Self {}
    }

    /// Matches a bot config option to a description string
    fn get_option_desc(&self, opt: &str) -> &str {
        match opt {
            "log" => "Settings controlling guild event logging",
            "log.log_bots" => "<Bool> Include bots in server activity logs",
            _ => "",
        }
    }
}

#[async_trait]
impl Command for Config {
    fn get_name(&self) -> &'static str {
        "config"
    }

    fn get_short(&self) -> &'static str {
        "Configures functions of the bot"
    }

    fn get_full(&self) -> &'static str {
        "Configures functions of the bot. \
        Available subcommands: list set get;\n \
        `list [group]` lists all groups/keys in a group\n \
        `set <group>.<key> <value>` sets a setting to value\n \
        `get <group>.<key>` gets the value of a setting \
        To clear a setting set its value to `none`."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::String("subcommand", true),
            CommandSyntax::String("argument1", false),
            CommandSyntax::String("argument2", false),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Admin
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        args: Vec<Token>,
        _params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        // find the config subcommand first
        let mut args_iter = args.into_iter();

        let Some(subcommand_token) = args_iter.next() else {
            return Err(CommandError::arg_not_found("String", Some("subcommand")));
        };
        let Token {
            contents: Some(CommandArgument::String(subcommand)),
            ..
        } = subcommand_token.clone()
        else {
            return Err(CommandError::arg_not_found("String", Some("subcommand")));
        };

        // process the first/second arg for the subcommand
        let arg1_token = args_iter.next();
        let arg1: Option<String> = match arg1_token.clone() {
            Some(arg) => match arg.contents {
                Some(CommandArgument::String(inner)) => Some(inner),
                _ => None,
            },
            None => None,
        };

        let arg2_token = args_iter.next();
        let arg2: Option<String> = match arg2_token.clone() {
            Some(arg) => match arg.contents {
                Some(CommandArgument::String(inner)) => Some(inner),
                _ => None,
            },
            None => None,
        };

        trace.point("fetching_guild_settings");
        // grab the settings struct
        let mut global = GUILD_SETTINGS.get().unwrap().lock().await;
        let settings = match global.get(msg.guild_id.map(|g| g.get()).unwrap_or(1)).await {
            Ok(s) => s,
            Err(_) => Settings::default(),
        };

        if subcommand == "list" {
            // the list subcommand which lists all available config options
            let Ok(Value::Object(json_rep)) = json::to_value(&settings) else {
                error!("Json serialization went wrong on guild settings");
                return Err(CommandError {
                    title: String::from("Could not fetch guild settings"),
                    hint: Some(String::from("please try again later")),
                    arg: None,
                });
            };

            let description = if let Some(group_key) = arg1 {
                let Some(Value::Object(group)) = json_rep.get(&group_key) else {
                    return Err(CommandError {
                        title: String::from("Could not find group"),
                        hint: Some(String::from("run `config list` for a list of all groups")),
                        arg: None,
                    });
                };

                format!(
                    "**Available Settings In Group**\n{}",
                    group
                        .keys()
                        .map(|k| format!(
                            "`{k}` - {}",
                            self.get_option_desc(format!("{group_key}.{k}").as_str())
                        ))
                        .collect::<Vec<String>>()
                        .join("\n")
                )
            } else {
                format!(
                    "**Available Config Groups**\n{}",
                    json_rep
                        .keys()
                        .map(|k| format!("`{k}` - {}", self.get_option_desc(k)))
                        .collect::<Vec<String>>()
                        .join("\n")
                )
            };

            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(description)
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("Could not send message; err = {err:?}");
            }

            Ok(())
        } else if subcommand == "get" {
            // the get subcommand which gets the current value of a config option
            let Some(setting) = arg1 else {
                return Err(CommandError::arg_not_found("String", Some("arg1")));
            };

            let value = match setting.as_str() {
                "log.log_bots" => settings
                    .log
                    .log_bots
                    .map(|c| format!("{c}"))
                    .unwrap_or(String::from("false")),
                _ => {
                    return Err(CommandError {
                        title: String::from("Could not find setting"),
                        hint: Some(String::from("run config list for a list of valid settings")),
                        arg: Some(arg1_token.unwrap()),
                    });
                }
            };

            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(format!("{setting}: {value}"))
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("Could not send message; err = {err:?}");
            }

            Ok(())
        } else if subcommand == "set" {
            trace.point("updating_guild_settings");
            // the set subcommand which sets the value of a config option
            let Some(setting) = arg1 else {
                return Err(CommandError::arg_not_found("String", Some("arg1")));
            };

            let Some(value) = arg2 else {
                return Err(CommandError::arg_not_found("String", Some("arg1")));
            };

            let mut iter = vec![arg2_token.clone().unwrap()].into_iter().peekable();

            // a map of config options to their correct argument transformers and sqlx statements
            let setting_info: (
                BoxedTransformerFn,
                sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
            ) = match setting.as_str() {
                "log.log_bots" => (
                    Box::new(Transformers::bool),
                    sqlx::query("UPDATE guild_settings SET log_bot = $2 WHERE guild_id = $1"),
                ),
                _ => {
                    return Err(CommandError {
                        title: String::from("Could not find setting"),
                        hint: Some(String::from(
                            "run `config list` for a list of valid settings",
                        )),
                        arg: Some(arg1_token.unwrap()),
                    });
                }
            };

            let res = if iter
                .peek()
                .map(|t| t.raw.clone())
                .unwrap_or_default()
                .to_lowercase()
                == "none"
            {
                setting_info
                    .1
                    .bind(msg.guild_id.map(|g| g.get()).unwrap_or(1) as i64)
                    .bind(None as Option<i32>)
                    .execute(&*SQL)
                    .await
            } else {
                match setting_info.0(&ctx, &msg, &mut iter).await {
                    Ok(Token {
                        contents: Some(CommandArgument::GuildChannel(channel)),
                        ..
                    }) => {
                        setting_info
                            .1
                            .bind(msg.guild_id.map(|g| g.get()).unwrap_or(1) as i64)
                            .bind(channel.id.get() as i64)
                            .execute(&*SQL)
                            .await
                    }

                    Ok(Token {
                        contents: Some(CommandArgument::bool(b)),
                        ..
                    }) => {
                        setting_info
                            .1
                            .bind(msg.guild_id.map(|g| g.get()).unwrap_or(1) as i64)
                            .bind(b)
                            .execute(&*SQL)
                            .await
                    }

                    Err(TransformerError::CommandError(mut err)) => {
                        err.arg = Some(arg2_token.unwrap());
                        return Err(err);
                    }

                    _ => {
                        return Err(CommandError {
                            title: String::from("Could not insert value into settings."),
                            hint: None,
                            arg: Some(arg2_token.unwrap()),
                        });
                    }
                }
            };

            if let Err(err) = res {
                warn!("Could not update guild settings; err = {err:?}");
                return Err(CommandError {
                    title: String::from("Could not update settings"),
                    hint: Some(String::from("please try again later.")),
                    arg: Some(arg1_token.unwrap()),
                });
            }

            global.invalidate();

            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(format!("Successfully set {setting} to {value}"))
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("Could not send message; err = {err:?}");
            }

            Ok(())
        } else {
            Err(CommandError {
                title: String::from("Subcommand not found"),
                hint: Some(String::from("available subcommands: list, get, set")),
                arg: Some(subcommand_token),
            })
        }
    }

    fn get_transformers(&self) -> Vec<TransformerFnArc> {
        vec![
            Arc::new(Transformers::some_string),
            Arc::new(Transformers::string),
            Arc::new(Transformers::some_string),
        ]
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::ADMINISTRATOR],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
