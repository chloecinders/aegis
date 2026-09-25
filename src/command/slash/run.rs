use std::sync::Arc;

use serenity::all::{
    CommandInteraction, CommandType, Context, CreateCommand, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::http::Http;
use tracing::{info, warn};

use crate::app::App;
use crate::command::Meta;
use crate::command::error::{Ctx as _, Error, Result};
use crate::command::permissions;
use crate::command::registry::ChatInput;
use crate::command::slash::cx::SlashCx;
use crate::command::slash::{Reply, definition};
use crate::platform::ui::error as render;
use crate::platform::ui::reply;

pub async fn handle_command(app: Arc<App>, ctx: Context, interaction: CommandInteraction) {
    let Some(guild) = interaction.guild_id else {
        return;
    };

    if !app.allows_guild(guild.get()) {
        return;
    }

    let Some(entry) = app.registry.find(&interaction.data.name).copied() else {
        return;
    };

    let Some(slash) = entry.slash else {
        return;
    };

    let echo = (ctx.clone(), interaction.clone());
    let attempt = tokio::spawn(run(app, ctx, interaction, entry.meta, slash));

    let Err(joined) = attempt.await else {
        return;
    };

    if !joined.is_panic() {
        return;
    }

    let (ctx, interaction) = echo;
    let failure = Error::internal("the command panicked");

    respond(
        &ctx,
        &interaction,
        Reply::new(render::render(&failure)).ephemeral(),
    )
    .await;
}

async fn run(
    app: Arc<App>,
    ctx: Context,
    interaction: CommandInteraction,
    meta: Meta,
    slash: ChatInput,
) {
    let mut cx = SlashCx::new(app, ctx, interaction, meta.name);

    let reply = match execute(&mut cx, &meta, slash).await {
        Ok(reply) => reply,
        Err(failure) => {
            cx.report(&failure);

            Reply::new(render::render(&failure)).ephemeral()
        }
    };

    respond(&cx.ctx, &cx.interaction, reply).await;
}

async fn execute(cx: &mut SlashCx, meta: &Meta, slash: ChatInput) -> Result<Reply> {
    permissions::check_invocation(cx, meta).await?;

    (slash.execute)(cx).await
}

async fn respond(ctx: &Context, interaction: &CommandInteraction, reply: Reply) {
    let sent = interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(reply.embed.build().into_owned())
                    .components(
                        reply
                            .buttons
                            .chunks(5)
                            .take(5)
                            .map(reply::row)
                            .collect::<Vec<_>>(),
                    )
                    .ephemeral(reply.ephemeral),
            ),
        )
        .await;

    if let Err(failure) = sent.ctx("answer slash command") {
        warn!("could not answer a slash command; err = {failure}");
    }
}

#[cfg(feature = "web")]
fn entrypoints(app: &App) -> Vec<CreateCommand<'static>> {
    match app.config.discord_client_id.is_some() {
        true => vec![
            crate::web::entrypoint::launcher(),
            crate::web::entrypoint::definition(),
        ],
        false => Vec::new(),
    }
}

#[cfg(not(feature = "web"))]
fn entrypoints(_app: &App) -> Vec<CreateCommand<'static>> {
    Vec::new()
}

pub async fn install(app: Arc<App>, http: Arc<Http>) {
    let wanted: Vec<CreateCommand<'static>> = app
        .registry
        .all()
        .iter()
        .filter(|entry| !entry.meta.developer)
        .filter_map(|entry| {
            entry
                .slash
                .map(|slash| definition(&entry.meta, slash.parameters))
        })
        .chain(entrypoints(&app))
        .collect();

    match http.create_global_commands(&wanted).await {
        Ok(registered) => info!(
            "registered {} slash commands",
            registered
                .iter()
                .filter(|command| command.kind == CommandType::ChatInput)
                .count()
        ),
        Err(failure) => warn!("could not register the slash commands; err = {failure}"),
    }
}
