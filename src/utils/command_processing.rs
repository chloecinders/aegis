use std::{collections::HashMap, sync::Arc};

use serenity::all::{Context, CreateAllowedMentions, CreateMessage, Message};
use tracing::warn;

use crate::{
    SQL,
    commands::{CommandArgument, TransformerError},
    event_handler::{CommandError, Handler},
    lexer::{Token, lex},
    utils::{
        cache::permission_cache::{CommandPermissionRequest, CommandPermissionResult},
        cache::trace_cache::CommandTrace,
        extract_command_parameters, is_developer,
        trace::TraceContext,
    },
};

pub async fn process(handler: &Handler, ctx: Context, mut msg: Message) {
    let contents = msg.content.clone();
    let strip = contents.strip_prefix(handler.prefix.as_str()).unwrap_or("");
    let tokens = lex(String::from(strip));
    let mut parts = tokens.into_iter().peekable();
    let command_name = parts.next().map(|s| s.raw).unwrap_or_default();

    if command_name == "help" {
        if let Err(err) = handler
            .help_run(ctx.clone(), msg.clone(), parts.collect())
            .await
        {
            handler.send_error(&ctx, &msg, contents, err).await;
        }

        return;
    } else if command_name == "cachedbg" && is_developer(&msg.author) {
        let lock = handler.message_cache.lock().await;
        let mut sizes = lock.get_sizes();
        let size = sizes.entry(msg.channel_id.get()).or_insert(100);
        let count = lock.get_channel_len(msg.channel_id.get());
        let mut inserts = lock.get_inserts();
        let insert_count = inserts.entry(msg.channel_id.get()).or_insert(0);

        let reply = CreateMessage::new()
            .content(format!(
                "Size: {}; Count: {}; Inserts: {}",
                *size, count, *insert_count
            ))
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            warn!("Could not send message; err = {err:?}");
        }

        return;
    }

    let command = handler
        .commands
        .iter()
        .find(|c| c.get_name() == command_name.to_lowercase());

    if let Some(c) = command {
        let mut trace = TraceContext::new();
        trace.point("lexing_and_finding");

        {
            let typing_http = ctx.http.clone();
            tokio::spawn(msg.channel_id.broadcast_typing(typing_http));
        }

        let mut parts = parts;

        let res: Result<(), CommandError> = async {
            let guild_id = msg.guild_id.ok_or_else(|| CommandError {
                title: String::from("You do not have permissions to execute this command."),
                hint: Some(String::from("could not get member object")),
                arg: None,
            })?;

            let bot_id = ctx.cache.current_user().id;

            let cached_context = {
                let cache_guild_arc = ctx.cache.guild(guild_id);
                let g_ref = cache_guild_arc.as_ref();

                let cache_member = g_ref.and_then(|g| g.members.get(&msg.author.id).cloned());
                let cache_channel = g_ref.and_then(|g| g.channels.get(&msg.channel_id).cloned());
                let cache_bot_member = g_ref.and_then(|g| g.members.get(&bot_id).cloned());

                if let (Some(member), Some(guild_arc), Some(channel), Some(bot_member)) =
                    (cache_member, cache_guild_arc, cache_channel, cache_bot_member)
                {
                    Some((
                        Ok(member),
                        Ok((
                            guild_arc.id,
                            guild_arc.owner_id,
                            Arc::new(guild_arc.roles.clone()),
                        )),
                        Ok(Some((channel.id, Arc::new(channel.permission_overwrites)))),
                        Ok(bot_member),
                    ))
                } else {
                    None
                }
            };

            let (member_res, guild_data_res, channel_data_res, current_user_res) =
                match cached_context {
                    Some(res) => {
                        trace.point("context_cache_hit");
                        res
                    }
                    None => {
                        trace.point("context_cache_miss");
                        let (m, g, c, b) = tokio::join!(
                            msg.member(&ctx),
                            guild_id.to_partial_guild(&ctx),
                            async { msg.channel(&ctx).await.map(|c| c.guild()) },
                            guild_id.member(&ctx, bot_id),
                        );

                        (
                            m,
                            g.map(|g| (g.id, g.owner_id, Arc::new(g.roles))),
                            c.map(|c| {
                                c.map(|c| (c.id, Arc::new(c.permission_overwrites)))
                            }),
                            b,
                        )
                    }
                };

            let member = member_res.map_err(|_| CommandError {
                title: String::from("You do not have permissions to execute this command."),
                hint: Some(String::from("could not get member object")),
                arg: None,
            })?;

            let (guild_id, owner_id, roles) = guild_data_res.map_err(|_| CommandError {
                title: format!("You do not have permissions to execute this command."),
                hint: Some(String::from("could not get guild object")),
                arg: None,
            })?;

            let (channel_id, overwrites) = channel_data_res
                .map_err(|_| CommandError {
                    title: format!("You do not have permissions to execute this command."),
                    hint: Some(String::from("could not get channel object")),
                    arg: None,
                })?
                .ok_or_else(|| CommandError {
                    title: format!("You do not have permissions to execute this command."),
                    hint: Some(String::from("could not get channel object")),
                    arg: None,
                })?;

            let current_user = current_user_res.map_err(|_| CommandError {
                title: format!("You do not have permissions to execute this command."),
                hint: Some(String::from("could not get current member object")),
                arg: None,
            })?;

            let request = CommandPermissionRequest {
                current_user,
                handler: handler.clone(),
                command: c.clone(),
                guild_id,
                owner_id,
                roles,
                channel_id,
                overwrites,
                member,
            };

            trace.point("fetch_context");

            let result = {
                let mut lock = handler.permission_cache.lock().await;
                lock.can_run(request, Some(&mut trace)).await
            };

            if result != CommandPermissionResult::Success {
                let err_msg = match result {
                    CommandPermissionResult::Success => unreachable!(),
                    CommandPermissionResult::FailedBot(perm) => format!(
                        "I do not have the required permissions to execute this command. Required: {perm}"
                    ),
                    CommandPermissionResult::FailedUserOneOf => String::from(
                        "You do not have one of the permissions required to execute this command.",
                    ),
                    CommandPermissionResult::FailedUserRequired => String::from(
                        "You do not have the required permissions to execute this command.",
                    ),
                    CommandPermissionResult::Uninitialised => {
                        String::from("You arent supposed to see this! Report this to the devs ;(")
                    }
                };

                return Err(CommandError {
                    title: err_msg,
                    hint: None,
                    arg: None,
                });
            }

            trace.point("permission_check");

            let mut command_params = HashMap::new();

            if !c.get_params().is_empty() {
                let params = c.get_params();
                let res = extract_command_parameters(&ctx, &msg, strip.to_string(), params).await;

                if let Ok(params) = res {
                    command_params = params.0;
                    let contents = format!("{}{}", handler.prefix, params.1.clone());
                    msg.content = contents;
                    parts = lex(params.1).into_iter().peekable();
                    parts.next();
                }
            }

            trace.point("extract_params");

            let mut transformers = c.get_transformers().into_iter();
            let mut args: Vec<Token> = vec![];

            while parts.peek().is_some() {
                if let Some(transformer) = transformers.next() {
                    let result = transformer(&ctx, &msg, &mut parts).await;

                    match result {
                        Ok(r) => {
                            args.push(r);
                        }
                        Err(TransformerError::MissingArgumentError(err)) => {
                            return Err(CommandError::arg_not_found(&err.0, None));
                        }
                        Err(TransformerError::CommandError(err)) => {
                            return Err(err);
                        }
                    }
                } else if let Some(mut arg) = parts.next() {
                    arg.contents = Some(CommandArgument::String(arg.raw.clone()));
                    args.push(arg);
                }
            }

            for transformer in transformers {
                let result = transformer(&ctx, &msg, &mut parts).await;

                match result {
                    Ok(r) => {
                        args.push(r);
                    }
                    Err(TransformerError::CommandError(err)) => {
                        return Err(err);
                    }
                    Err(TransformerError::MissingArgumentError(_)) => {
                        args.push(Token {
                            contents: Some(CommandArgument::None),
                            raw: String::new(),
                            position: 0,
                            length: 0,
                            iteration: 0,
                            quoted: false,
                            inferred: None,
                        });
                    }
                }
            }

            trace.point("transform_args");

            let res = c
                .run(
                    ctx.clone(),
                    msg.clone(),
                    handler,
                    args,
                    command_params,
                    &mut trace,
                )
                .await;

            trace.point("execution");
            res
        }
        .await;

        let trace_record = CommandTrace {
            message_id: msg.id,
            command_name: c.get_name().to_string(),
            total_duration: trace.start_time().elapsed(),
            points: trace.points,
            success: res.is_ok(),
            error: match &res {
                Err(err) => Some(err.title.clone()),
                Ok(_) => None,
            },
        };

        tokio::spawn(async move {
            if let Err(err) = sqlx::query!(
                "INSERT INTO command_traces (message_id, command_name, total_duration_nanos, success, error, points) VALUES ($1, $2, $3, $4, $5, $6)",
                trace_record.message_id.get() as i64,
                trace_record.command_name,
                trace_record.total_duration.as_nanos() as i64,
                trace_record.success,
                trace_record.error,
                serde_json::to_value(&trace_record.points).unwrap_or_default()
            )
            .execute(&*SQL)
            .await
            {
                warn!("Could not save command trace to database; err = {err:?}");
            }
        });

        if let Err(err) = res {
            handler
                .send_error(&ctx, &msg, msg.content.clone(), err)
                .await;
        }
    }
}
