use std::sync::Arc;

use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message, Permissions},
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
    utils::{consume_pgsql_error, consume_serenity_error, rule_cache::OcrPattern},
};
use aegis_macros::command;
use sqlx::Row;

pub struct AddOcrPattern;

impl AddOcrPattern {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for AddOcrPattern {
    fn get_name(&self) -> &'static str {
        "add_ocr_pattern"
    }

    fn get_short(&self) -> &'static str {
        "Adds a pattern to an existing OCR rule"
    }

    fn get_full(&self) -> &'static str {
        "Appends a new keyword or regex pattern to an existing OCR automod rule. \
        Wrap a pattern in slashes (e.g. /someregex/) to treat it as a Rust Regex; \
        otherwise plain fuzzy string matching is used (case insensitive). \
        Multiple patterns on the same rule are evaluated with OR logic — the rule \
        fires when any one of them matches."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::String("rule_id", true),
            CommandSyntax::String("pattern", true),
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
        #[transformers::some_string] rule_id: String,
        #[transformers::some_string] pattern: String,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let Some(guild_id) = msg.guild_id else {
            return Err(CommandError::new("Could not get guild id"));
        };

        if pattern.len() >= 500 {
            return Err(CommandError {
                title: String::from("pattern argument can only be a max of 500 characters long"),
                hint: None,
                arg: Some(_pattern_arg),
            });
        }

        let (inner, is_regex) =
            if let Some(stripped) = pattern.strip_prefix('/').and_then(|s| s.strip_suffix('/')) {
                (stripped.to_string(), true)
            } else {
                (pattern.clone(), false)
            };

        if is_regex {
            if let Err(e) = regex::Regex::new(&inner) {
                return Err(CommandError {
                    title: format!("Invalid regex: {e}"),
                    hint: Some(String::from("use Rust regex syntax")),
                    arg: Some(_pattern_arg),
                });
            }
        }

        trace.point("fetching_rule");
        let row = sqlx::query(
            "SELECT id, name, patterns FROM automod_rules WHERE id = $1 AND guild_id = $2 AND type = 'ocr'",
        )
        .bind(&rule_id)
        .bind(guild_id.get() as i64)
        .fetch_optional(&*SQL)
        .await;

        let row = match row {
            Ok(Some(r)) => r,
            Ok(None) => {
                return Err(CommandError {
                    title: format!("No OCR rule with ID `{rule_id}` found in this server"),
                    hint: Some(String::from("use +rules to list all rules")),
                    arg: Some(_rule_id_arg),
                });
            }
            Err(err) => {
                consume_pgsql_error("ADD OCR PATTERN FETCH".into(), err);
                return Err(CommandError {
                    title: String::from("Could not query the database"),
                    hint: Some(String::from("please try again later")),
                    arg: None,
                });
            }
        };

        let rule_name: String = row.get("name");

        trace.point("updating_database");
        let new_entry = serde_json::json!({ "pattern": inner, "is_regex": is_regex });

        if let Err(err) = sqlx::query(
            "UPDATE automod_rules \
             SET patterns = patterns || $1::jsonb \
             WHERE id = $2",
        )
        .bind(sqlx::types::Json(serde_json::json!([new_entry])))
        .bind(&rule_id)
        .execute(&*SQL)
        .await
        {
            consume_pgsql_error("ADD OCR PATTERN UPDATE".into(), err);
            return Err(CommandError {
                title: String::from("Could not update the database"),
                hint: None,
                arg: None,
            });
        }

        {
            let mut lock = handler.rule_cache.lock().await;
            lock.add_pattern_to_rule(
                &rule_id,
                OcrPattern {
                    pattern: inner.clone(),
                    is_regex,
                },
            );
        }

        let updated_patterns: Vec<String> = {
            let cache = handler.rule_cache.lock().await;
            cache
                .get_by_id(&rule_id)
                .map(|r| {
                    r.patterns
                        .iter()
                        .map(|p| {
                            if p.is_regex {
                                format!("`/{}/` [regex]", p.pattern)
                            } else {
                                format!("`{}` [plain]", p.pattern)
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let pattern_display = if updated_patterns.is_empty() {
            pattern.clone()
        } else {
            updated_patterns.join("\n")
        };

        let type_label = if is_regex { "regex" } else { "plain" };
        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**ADDED PATTERN TO OCR RULE {}**\n\
                         -# ID: `{}` | New pattern: `{}` [{}]\n\
                         **All patterns:**\n{}",
                        rule_name.to_uppercase(),
                        rule_id,
                        inner,
                        type_label,
                        pattern_display,
                    ))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            consume_serenity_error("ADD OCR PATTERN RESPONSE".into(), err);
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
