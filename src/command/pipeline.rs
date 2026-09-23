use std::sync::Arc;

use serenity::all::{Context, Message, MessageId};

use crate::app::App;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::registry::Entry;
use crate::command::stream::Stream;
use crate::command::typing::Typing;
use crate::command::{EditMode, Response, permissions};
use crate::domain::ids::ActionId;
use crate::features::diagnostics::store::TraceRow;
use crate::features::records::store as invocations;
use crate::features::snippets;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::{error as render, reply};

fn invocable(app: &App, msg: &Message) -> bool {
    if msg.author.bot() || !msg.content.starts_with(app.prefix()) {
        return false;
    }

    match msg.guild_id {
        Some(guild) => app.allows_guild(guild.get()),
        None => true,
    }
}

pub async fn guarded(app: Arc<App>, ctx: Context, msg: Arc<Message>) {
    if !invocable(&app, &msg) {
        return;
    }

    let echo = (ctx.clone(), Arc::clone(&msg));
    let attempt = tokio::spawn(handle(app, ctx, msg));

    let Err(joined) = attempt.await else {
        return;
    };

    if !joined.is_panic() {
        return;
    }

    let (ctx, msg) = echo;
    let failure = Error::internal("the command panicked").against(msg.content.as_str());
    let embed = render::render(&failure);

    let _ = msg
        .channel_id
        .send_message(&ctx.http, reply::plain(&embed).reference_message(&*msg))
        .await;
}

pub async fn handle(app: Arc<App>, ctx: Context, msg: Arc<Message>) {
    if !invocable(&app, &msg) {
        return;
    }

    let Some(input) = snippets::line(&app, &msg).await else {
        return;
    };

    run(app, ctx, msg, input, None).await;
}

pub async fn run(
    app: Arc<App>,
    ctx: Context,
    msg: Arc<Message>,
    input: Arc<str>,
    revising: Option<MessageId>,
) {
    let mut stream = Stream::new(Arc::clone(&input), app.prefix().len());
    let Some(invoked) = stream.advance() else {
        return;
    };

    let Some(entry) = app.registry.find(&invoked.raw).copied() else {
        return;
    };

    let mut cx =
        Cx::reading(Arc::clone(&app), ctx.clone(), Arc::clone(&msg), input).amending(revising);

    cx.trace("resolve");
    open(&cx, &entry).await;

    let outcome = execute(&mut cx, &entry, &mut stream).await;
    let verdict = match &outcome {
        Ok(_) => None,
        Err(failure) => Some(failure.headline().to_string()),
    };

    let action = cx.action();
    let failed = verdict.is_some();
    let response = deliver(&cx, outcome).await;

    record(&cx, &entry, verdict);
    close(&cx, &entry, action, response, failed).await;
}

async fn open(cx: &Cx, entry: &Entry) {
    let Ok(guild) = cx.guild_snowflake() else {
        return;
    };

    let written = invocations::remember_invocation(
        cx.pool(),
        guild,
        cx.channel_id().get(),
        cx.msg.id.get(),
        cx.author_id().get(),
        entry.meta.name,
        &serde_json::Value::Null,
    )
    .await;

    if let Err(failure) = written {
        cx.report(&failure);
    }
}

async fn close(
    cx: &Cx,
    entry: &Entry,
    action: Option<ActionId>,
    response: Option<MessageId>,
    failed: bool,
) {
    if cx.guild_snowflake().is_err() {
        return;
    }

    let retractable = failed || entry.meta.edit != EditMode::Fixed;
    let record = cx.invocation();
    let written = invocations::close_invocation(
        cx.pool(),
        cx.msg.id.get(),
        record.as_ref().map(|record| &record.args),
        action.as_ref(),
        retractable
            .then(|| response.or(cx.revision()))
            .flatten()
            .map(|id| id.get()),
        failed,
    )
    .await;

    if let Err(failure) = written {
        cx.report(&failure);
    }
}

async fn execute(cx: &mut Cx, entry: &Entry, stream: &mut Stream) -> Result<Response> {
    permissions::statics(cx, &entry.meta).await?;
    cx.trace("gate_static");

    let _signal = Typing::watch(&cx.ctx, cx.channel_id());

    (entry.execute)(cx, stream).await
}

async fn post(cx: &Cx, embed: &Embed) -> Option<MessageId> {
    match cx.present(embed, Vec::new(), "send command response").await {
        Ok(posted) => Some(posted),
        Err(failure) => {
            cx.report(&failure);

            None
        }
    }
}

async fn deliver(cx: &Cx, outcome: Result<Response>) -> Option<MessageId> {
    let failure = match outcome {
        Ok(Response::None) => return None,
        Ok(Response::Sent(id)) => return Some(id),
        Ok(Response::Embed(embed)) => return post(cx, &embed).await,
        Err(failure) => failure,
    };

    cx.report(&failure);

    post(cx, &render::render(&failure)).await
}

fn record(cx: &Cx, entry: &Entry, verdict: Option<String>) {
    let trace = cx.trace_snapshot();
    let row = TraceRow {
        message: cx.msg.id.get(),
        command: entry.meta.name,
        nanos: trace.elapsed().as_nanos() as i64,
        success: verdict.is_none(),
        failure: verdict,
        points: trace.as_json(),
    };

    cx.app.traces.send(row);
}
