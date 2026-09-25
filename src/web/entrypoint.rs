use serenity::all::{
    CommandInteraction, CommandType, Context, CreateCommand, CreateInteractionResponse,
    EntryPointHandlerType,
};
use tracing::warn;

pub fn definition() -> CreateCommand<'static> {
    CreateCommand::new("configure")
        .kind(CommandType::ChatInput)
        .description("Open the Aegis dashboard for this server")
        .dm_permission(false)
}

pub fn launcher() -> CreateCommand<'static> {
    CreateCommand::new("launch")
        .kind(CommandType::PrimaryEntryPoint)
        .handler(EntryPointHandlerType::DiscordLaunchActivity)
        .description("Open Aegis")
}

pub async fn launched(ctx: &Context, interaction: &CommandInteraction) {
    if interaction.data.name != "configure" {
        return;
    }

    let sent = interaction
        .create_response(&ctx.http, CreateInteractionResponse::LaunchActivity)
        .await;

    if let Err(failure) = sent {
        warn!("could not launch the dashboard; err = {failure}");
    }
}
