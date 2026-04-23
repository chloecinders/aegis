use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
    utils::{consume_serenity_error, is_developer},
};
use ouroboros_macros::command;

pub struct Jeprof;

impl Jeprof {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Jeprof {
    fn get_name(&self) -> &'static str {
        "jeprof"
    }

    fn get_short(&self) -> &'static str {
        "Gets heap profile data summary"
    }

    fn get_full(&self) -> &'static str {
        "Evaluates the latest jemalloc .heap file using jeprof and shows a summary."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        use crate::transformers::Transformers;
        vec![&CommandParameter {
            name: "raw",
            short: "r",
            transformer: &Transformers::none,
            desc: "Uploads the raw .heap file instead of running jeprof",
        }]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        _handler: &Handler,
        _args: Vec<Token>,
        params: std::collections::HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if is_developer(&msg.author) {
            let raw = params.contains_key("raw");
            trace.point("finding_latest_heap");

            let mut latest_heap = None;
            let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

            if let Ok(entries) = std::fs::read_dir(".") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("heap") {
                        if let Ok(meta) = entry.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if modified > latest_time {
                                    latest_time = modified;
                                    latest_heap = Some(path);
                                }
                            }
                        }
                    }
                }
            }

            let Some(heap_path) = latest_heap else {
                let _ = msg
                    .channel_id
                    .say(&ctx, "No `.heap` files found in the working directory.")
                    .await;
                return Ok(());
            };

            if raw {
                trace.point("uploading_raw_heap");

                let attachment = match serenity::all::CreateAttachment::path(&heap_path).await {
                    Ok(a) => a,
                    Err(e) => {
                        let _ = msg
                            .channel_id
                            .say(&ctx, format!("Failed to attach heap file: {}", e))
                            .await;
                        return Ok(());
                    }
                };

                let filename = heap_path.file_name().unwrap_or_default().to_string_lossy();
                let description = format!("**JEPROF RAW HEAP PROFILE**\nFile: `{}`", filename);

                let embed = CreateEmbed::new()
                    .description(description)
                    .color(BRAND_BLUE);

                let r = CreateMessage::new()
                    .add_embed(embed)
                    .reference_message(&msg)
                    .add_file(attachment)
                    .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

                if let Err(err) = msg.channel_id.send_message(&ctx, r).await {
                    consume_serenity_error(String::from("JEPROF RAW UPLOAD"), err);
                }
            } else {
                trace.point("running_jeprof");

                let exe = std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("./Ouroboros"));

                let output = match std::process::Command::new("jeprof")
                    .arg("--text")
                    .arg(&exe)
                    .arg(&heap_path)
                    .output()
                {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = msg
                            .channel_id
                            .say(&ctx, format!("Failed to run jeprof: {}", e))
                            .await;
                        return Ok(());
                    }
                };

                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    stdout = format!("jeprof failed!\nstdout: {}\nstderr: {}", stdout, stderr);
                }

                trace.point("uploading_heap_summary");

                let attachment = serenity::all::CreateAttachment::bytes(
                    stdout.into_bytes(),
                    "jeprof_summary.txt",
                );

                let filename = heap_path.file_name().unwrap_or_default().to_string_lossy();
                let description = format!(
                    "**JEPROF OVERVIEW**\nTarget: `{}`\nFull analysis attached below.",
                    filename
                );

                let embed = CreateEmbed::new()
                    .description(description)
                    .color(BRAND_BLUE);

                let r = CreateMessage::new()
                    .add_embed(embed)
                    .reference_message(&msg)
                    .add_file(attachment)
                    .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

                if let Err(err) = msg.channel_id.send_message(&ctx, r).await {
                    consume_serenity_error(String::from("JEPROF SUMMARY UPLOAD"), err);
                }
            }
        }

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
