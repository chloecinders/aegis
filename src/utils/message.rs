use std::{collections::HashMap, sync::Arc, time::Duration};

use serenity::all::{
    Context, CreateAllowedMentions, CreateEmbed, CreateMessage, EditMessage, Message, UserId,
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};
use tracing::warn;

use crate::{
    commands::{CommandArgument, CommandParameter, TransformerError},
    constants::BRAND_BLUE,
    lexer::{Token, lex},
    utils::{TraceContext, reference::RefData},
};

pub struct CommandMessageResponse {
    server_content: Box<dyn Fn(String) -> String + Send + Sync>,
    dm_content: String,
    user: UserId,
    delete: bool,
    join_thread: Arc<Mutex<Option<JoinHandle<bool>>>>,
    dm_result: Arc<Mutex<Option<bool>>>,
    silent: bool,
    ref_raw: Option<RefData>,
    note: Option<String>,
}

impl CommandMessageResponse {
    pub fn new(user_id: UserId) -> Self {
        Self {
            server_content: Box::new(|a| a),
            dm_content: String::default(),
            user: user_id,
            delete: false,
            join_thread: Arc::new(Mutex::new(None)),
            dm_result: Arc::new(Mutex::new(None)),
            silent: false,
            ref_raw: None,
            note: None,
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
    pub fn ref_data(mut self, ref_data: RefData) -> Self {
        self.ref_raw = Some(ref_data);
        self
    }
    pub fn note(mut self, note: Option<String>) -> Self {
        self.note = note;
        self
    }

    pub async fn send_dm(&self, ctx: &Context) {
        if self.silent {
            return;
        }

        let ctx_clone = ctx.clone();
        let desc = self.dm_content.clone();
        let user = self.user.clone();

        let mut lock = self.join_thread.lock().await;
        *lock = Some(tokio::spawn(async move {
            let embed = CreateEmbed::new().description(desc).color(BRAND_BLUE);
            let dm = CreateMessage::new().add_embed(embed);
            user.direct_message(&ctx_clone, dm).await.is_ok()
        }));
    }

    pub async fn wait_for_dm(&self) -> bool {
        if self.silent {
            return true;
        }

        let mut lock = self.join_thread.lock().await;
        if let Some(handle) = lock.take() {
            let res = handle.await.unwrap_or(false);
            let mut res_lock = self.dm_result.lock().await;
            *res_lock = Some(res);
            res
        } else {
            let res_lock = self.dm_result.lock().await;
            res_lock.unwrap_or(true)
        }
    }

    pub async fn send_response(
        &mut self,
        ctx: &Context,
        cmd_msg: &Message,
        trace: &mut TraceContext,
    ) {
        let mut addition = if self.silent {
            String::from(" | silent")
        } else {
            let mut lock = self.join_thread.lock().await;
            trace.point("inspecting_dm_thread");

            if let Some(h) = lock.as_ref() {
                if h.is_finished() {
                    let handle = lock.take().unwrap();
                    let res = handle.await.unwrap_or(false);
                    let mut res_lock = self.dm_result.lock().await;
                    *res_lock = Some(res);
                }
            }

            let res_lock = self.dm_result.lock().await;
            match *res_lock {
                Some(true) | None => String::new(),
                Some(false) => String::from(" | DM failed"),
            }
        };

        if let Some(ref_data) = &self.ref_raw {
            let has_content = ref_data.content.is_some();
            let has_image = ref_data.image_url.is_some();
            if has_content && has_image {
                addition.push_str(" | + ref, + image");
            } else if has_content {
                addition.push_str(" | + ref");
            } else if has_image {
                addition.push_str(" | + image");
            }
        }

        let mut description = (*self.server_content)(addition);
        if let Some(note) = &self.note {
            description.push_str(&format!("\n-# {note}"));
        }

        let embed = CreateEmbed::new()
            .description(description)
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

            let dm_success = match handle.await {
                Ok(b) => b,
                Err(_) => false,
            };
            let mut res_lock = self.dm_result.lock().await;
            *res_lock = Some(dm_success);

            if !dm_success {
                let mut addition = String::from(" | DM failed");

                if let Some(ref_data) = &self.ref_raw {
                    let has_content = ref_data.content.is_some();
                    let has_image = ref_data.image_url.is_some();

                    if has_content && has_image {
                        addition.push_str(" | + ref, + image");
                    } else if has_content {
                        addition.push_str(" | + ref");
                    } else if has_image {
                        addition.push_str(" | + image");
                    }
                }

                let desc = (*self.server_content)(addition);
                let embed = CreateEmbed::new().description(desc).color(BRAND_BLUE);
                trace.point("editing_response");
                let _ = msg.edit(&ctx, EditMessage::new().embeds(vec![embed])).await;
            }
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

fn token_span(token: &Token, contents: &str) -> (usize, usize) {
    let mut start = token.position;
    let mut end = token.position + token.length;

    if token.quoted {
        let bytes = contents.as_bytes();
        if start > 0
            && (bytes.get(start - 1) == Some(&b'"') || bytes.get(start - 1) == Some(&b'\''))
        {
            start -= 1;
        }
        if bytes.get(end) == Some(&b'"') || bytes.get(end) == Some(&b'\'') {
            end += 1;
        }
    }

    (start, end)
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

                found_args.insert(param.name, (positive, contents_arg));

                if lex.len() == cloned.len() {
                    to_remove.push(token_span(&token, &contents));
                } else {
                    let num_consumed = cloned.len() - lex.len();
                    let last_consumed = cloned.clone().nth(num_consumed - 1);
                    let start = token_span(&token, &contents).0;
                    let end = last_consumed
                        .as_ref()
                        .map(|t| token_span(t, &contents).1)
                        .unwrap_or_else(|| token_span(&token, &contents).1);
                    to_remove.push((start, end));
                }
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
