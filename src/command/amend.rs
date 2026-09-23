use std::sync::Arc;

use chrono::Utc;
use serenity::all::{Context, EditMessage, Message, MessageId};

use crate::app::App;
use crate::command::cx::Cx;
use crate::command::edit::{Verdict, compare};
use crate::command::error::{Ctx as _, Error, Result};
use crate::command::registry::Entry;
use crate::command::stream::Stream;
use crate::command::{EditMode, Response, permissions, pipeline};
use crate::features::records::{amend, store};
use crate::features::snippets;
use crate::platform::ui::error as render;

pub async fn reconsider(app: Arc<App>, ctx: Context, msg: Arc<Message>) {
    let Ok(Some(record)) = store::load_invocation(&app.pool, msg.id.get()).await else {
        return;
    };

    if record.author != msg.author.id.get() {
        return;
    }

    if Utc::now().signed_duration_since(record.created_at) > app.config.edit_window() {
        return;
    }

    let Some(input) = snippets::line(&app, &msg).await else {
        return;
    };

    let mut stream = Stream::new(Arc::clone(&input), app.prefix().len());
    let Some(invoked) = stream.advance() else {
        return;
    };

    let Some(entry) = app.registry.find(&invoked.raw).copied() else {
        return;
    };

    if entry.meta.edit == EditMode::Fixed {
        return;
    }

    let response = record.response.map(MessageId::new);
    let renamed = entry.meta.name != record.command;

    if renamed || entry.meta.edit == EditMode::Rerun || record.status == "failed" {
        if renamed && record.action.is_some() {
            return;
        }

        pipeline::run(app, ctx, msg, input, response).await;

        return;
    }

    let mut cx = Cx::reading(Arc::clone(&app), ctx, Arc::clone(&msg), input).amending(response);
    let outcome = amended(&mut cx, &entry, &record, &mut stream).await;

    respond(&cx, response, outcome).await;
}

async fn amended(
    cx: &mut Cx,
    entry: &Entry,
    record: &store::Invocation,
    stream: &mut Stream,
) -> Result<Response> {
    permissions::statics(cx, &entry.meta).await?;

    let revised = (entry.rehearse)(cx, stream).await?;

    match compare(entry.fields, &record.args, &revised) {
        Verdict::Unchanged => Ok(Response::None),
        Verdict::Reject(why) => Err(Error::internal(why)),
        Verdict::Amend(changes) => {
            cx.remember(entry.meta.name, revised.clone());

            let Some(id) = record.action.as_ref() else {
                return Err(Error::bare().title("log not found"));
            };

            let guild = cx.guild_snowflake()?;

            let Some(action) = store::load(cx.pool(), guild, id).await? else {
                return Err(Error::bare().title("log not found"));
            };

            amend::inline(cx, &action, &changes, record.response).await?;
            store::remember_invocation(
                cx.pool(),
                guild,
                cx.channel_id().get(),
                cx.msg.id.get(),
                cx.author_id().get(),
                entry.meta.name,
                &revised,
            )
            .await?;

            Ok(Response::None)
        }
    }
}

async fn respond(cx: &Cx, response: Option<MessageId>, outcome: Result<Response>) {
    let embed = match outcome {
        Ok(Response::None) | Ok(Response::Sent(_)) => return,
        Ok(Response::Embed(embed)) => *embed,
        Err(failure) => {
            cx.report(&failure);
            render::render(&failure)
        }
    };

    let Some(response) = response else {
        return;
    };

    let edited = cx
        .channel_id()
        .edit_message(
            &cx.ctx.http,
            response,
            EditMessage::new()
                .embeds(vec![embed.build()])
                .components(Vec::new()),
        )
        .await;

    if let Err(failure) = edited.ctx("edit response after amendment") {
        cx.report(&failure);
    }
}
