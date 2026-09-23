use serenity::all::{
    Command, CommandId, CommandInteraction, CommandType, Context, CreateCommand,
    CreateInteractionResponse, EntryPointHandlerType,
};
use serenity::http::Http;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    Create,
    Rename(CommandId),
    None,
}

pub fn slash_work(existing: &[Command]) -> Work {
    match existing
        .iter()
        .find(|command| command.kind == CommandType::ChatInput && command.name == "configure")
    {
        Some(_) => Work::None,
        None => Work::Create,
    }
}

pub fn launcher_work(existing: &[Command]) -> Work {
    match existing
        .iter()
        .find(|command| command.kind == CommandType::PrimaryEntryPoint)
    {
        Some(found) if found.name == "launch" => Work::None,
        Some(found) => Work::Rename(found.id),
        None => Work::Create,
    }
}

pub async fn install(http: &Http) {
    let existing = match http.get_global_commands().await {
        Ok(existing) => existing,
        Err(failure) => {
            warn!("could not read the registered commands; err = {failure}");

            return;
        }
    };

    apply(http, launcher_work(&existing), &launcher(), "launch").await;
    apply(http, slash_work(&existing), &definition(), "configure").await;
}

async fn apply(http: &Http, work: Work, wanted: &CreateCommand<'_>, name: &str) {
    let outcome = match work {
        Work::None => return,
        Work::Create => http.create_global_command(wanted).await,
        Work::Rename(id) => http.edit_global_command(id, wanted).await,
    };

    if let Err(failure) = outcome {
        warn!("could not register /{name}; err = {failure}");
    }
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
