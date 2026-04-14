use std::{collections::HashMap, sync::Arc, time::Duration};

use serenity::{
    FutureExt,
    all::{
        Context, CreateAllowedMentions, CreateEmbed, CreateMessage, EditMessage, Message, UserId,
    },
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};
use tracing::warn;

use crate::{
    commands::{CommandArgument, CommandParameter, TransformerError},
    constants::BRAND_BLUE,
    lexer::lex,
};

pub struct CommandMessageResponse {
    server_content: Box<dyn Fn(String) -> String + Send + Sync>,
    dm_content: String,
    user: UserId,
    delete: bool,
    join_thread: Arc<Mutex<Option<JoinHandle<bool>>>>,
    silent: bool,
}

impl CommandMessageResponse {
    pub fn new(user_id: UserId) -> Self {
        Self {
            server_content: Box::new(|a| a),
            dm_content: String::default(),
            user: user_id,
            delete: false,
            join_thread: Arc::new(Mutex::new(None)),
            silent: false,
        }
    }

    pub fn server_content(mut self, content: Box<dyn Fn(String) -> String + Send + Sync>) -> Self {
        self.server_content = content;
        self
    }

    pub fn dm_content(mut self, content: String) -> Self {
        self.dm_content = content;
        self
    }

    pub fn automatically_delete(mut self, delete: bool) -> Self {
        self.delete = delete;
        self
    }

    pub fn mark_silent(mut self, silent: bool) -> Self {
        self.silent = silent;
        self
    }

    pub async fn send_dm(&self, ctx: &Context) {
        let ctx_clone = ctx.clone();
        let desc = self.dm_content.clone();
        let user = self.user.clone();

        {
            let mut lock = self.join_thread.lock().await;

            *lock = Some(tokio::spawn(async move {
                let dm = CreateMessage::new()
                    .add_embed(CreateEmbed::new().description(desc).color(BRAND_BLUE));

                user.direct_message(&ctx_clone, dm).await.is_err()
            }));
        }
    }

    pub async fn send_response(&mut self, ctx: &Context, cmd_msg: &Message, trace: &mut crate::utils::TraceContext) {
        let addition = if self.silent {
            String::from("| silent")
        } else {
            let mut lock = self.join_thread.lock().await;
            trace.point("inspecting_dm_thread");

            let mut finished = false;
            let res = match lock.as_mut() {
                Some(h) if h.is_finished() => {
                    finished = true;
                    match h.now_or_never() {
                        Some(Ok(b)) if b => String::new(),
                        _ => String::from(" | DM failed"),
                    }
                }
                _ => String::from(" | DM failed"),
            };

            if finished {
                lock.take();
            }
            res
        };

        let embed = CreateEmbed::new()
            .description((*self.server_content)(addition))
            .color(BRAND_BLUE);

        let reply = CreateMessage::new()
            .add_embed(embed)
            .reference_message(cmd_msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        trace.point("sending_initial_response");
        let mut msg = match cmd_msg.channel_id.send_message(&ctx, reply).await {
            Ok(m) => m,
            Err(err) => {
                warn!("Could not send message; err = {err:?}");
                return;
            }
        };

        let mut lock = self.join_thread.lock().await;
        if let Some(handle) = lock.take() {
            trace.point("waiting_for_dm_completion");
            let addition = match handle.await {
                Ok(b) if b => String::new(),
                _ => String::from("| DM failed"),
            };
            let desc = (*self.server_content)(addition);

            trace.point("editing_response");
            let _ = msg
                .edit(
                    &ctx,
                    EditMessage::new()
                        .embed(CreateEmbed::new().description(desc).color(BRAND_BLUE)),
                )
                .await;
        }

        if self.delete {
            let ctx = ctx.clone();
            let cmd_msg = cmd_msg.clone();

            trace.point("spawning_deletion_task");
            tokio::spawn(async move {
                sleep(Duration::from_secs(5)).await;
                tokio::join!(msg.delete(&ctx), cmd_msg.delete(&ctx))
            });
        }
    }
}

pub async fn extract_command_parameters<'a>(
    context: &Context,
    msg: &Message,
    contents: String,
    params: Vec<&CommandParameter<'static>>,
) -> Result<(HashMap<&'a str, (bool, CommandArgument)>, String), TransformerError> {
    let mut found_args: HashMap<&str, (bool, CommandArgument)> = HashMap::default();
    let mut lex = lex(contents.clone()).into_iter().peekable();
    let mut to_remove = Vec::new();

    while let Some(token) = lex.next() {
        let Some((positive, arg_name)) = ({
            token
                .raw
                .strip_prefix("-")
                .map(|a| (false, a))
                .or(token.raw.strip_prefix("+").map(|a| (true, a)))
        }) else {
            continue;
        };

        for param in params.iter() {
            if param.name == arg_name || param.short == arg_name {
                let cloned = lex.clone();
                let contents_arg = (*param.transformer)(context, msg, &mut lex)
                    .await
                    .map(|t| t.contents.unwrap_or(CommandArgument::None))
                    .unwrap_or(CommandArgument::None);

                if lex.len() == cloned.len() {
                    to_remove.push((token.position, token.position + token.length));
                    found_args.insert(param.name, (positive, contents_arg));
                    continue;
                }

                let mut last_consumed = None;
                let mut cloned_iter = cloned.rev().into_iter();
                let pos_to_search = lex.peek().map(|t| t.position).unwrap_or(0);

                while let Some(token) = cloned_iter.next() {
                    let curr_pos = token.position;
                    last_consumed = Some(token);

                    if curr_pos == pos_to_search {
                        last_consumed = Some(cloned_iter.next().or(last_consumed).unwrap());
                        break;
                    }
                }

                found_args.insert(param.name, (positive, contents_arg));
                let last_position = last_consumed
                    .clone()
                    .map(|t| t.position)
                    .unwrap_or(token.position);
                let last_length = last_consumed.map(|t| t.length).unwrap_or(token.length);
                to_remove.push((token.position, last_position + last_length));
            }
        }
    }

    to_remove.sort_by_key(|r| r.0);
    let mut stripped = String::new();
    let mut last_end = 0;
    for (start, end) in to_remove {
        if start > last_end {
            stripped.push_str(&contents[last_end..start]);
        }
        last_end = end;
    }
    if last_end < contents.len() {
        stripped.push_str(&contents[last_end..]);
    }

    Ok((found_args, stripped.trim().to_string()))
}
