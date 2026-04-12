use serenity::{
    all::{Context, CreateEmbed, CreateMessage, Message, Permissions},
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
    utils::{
        consume_serenity_error,
        ocr::{ImageData, image_to_string},
    },
};
use ouroboros_macros::command;

pub struct OcrCheck;

impl OcrCheck {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for OcrCheck {
    fn get_name(&self) -> &'static str {
        "ocr_check"
    }

    fn get_short(&self) -> &'static str {
        "Get OCR output of an image."
    }

    fn get_full(&self) -> &'static str {
        "Runs an image through OCR and sends the output. Useful for determining OCR automoderation rules."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(&self, ctx: Context, msg: Message, trace: &mut crate::utils::TraceContext) -> Result<(), CommandError> {
        let Some(attachment) = msg.attachments.first() else {
            return Err(CommandError {
                title: String::from("missing attachment"),
                hint: None,
                arg: None,
            });
        };

        trace.point("fetching_image");
        let Ok(req) = reqwest::get(attachment.proxy_url.clone()).await else {
            return Err(CommandError {
                title: String::from("failed to download provided attachment"),
                hint: None,
                arg: None,
            });
        };
        let Ok(bytes) = req.bytes().await else {
            return Err(CommandError {
                title: String::from("failed to download provided attachment"),
                hint: None,
                arg: None,
            });
        };
        let Ok(img) = image::load_from_memory(&bytes) else {
            return Err(CommandError {
                title: String::from("failed to decode provided attachment"),
                hint: None,
                arg: None,
            });
        };

        let img = img.to_rgba8();

        let image_data = ImageData {
            width: img.width().try_into().unwrap_or(0),
            height: img.height().try_into().unwrap_or(0),
            raw: img.into_raw(),
        };

        trace.point("processing_ocr");
        let image_str = match image_to_string(&image_data).await {
            Ok(d) => d,
            Err(_) => {
                return Err(CommandError {
                    title: String::from("failed to ocr provided attachment"),
                    hint: None,
                    arg: None,
                });
            }
        };

        if let Err(e) = msg
            .channel_id
            .send_message(
                &ctx,
                CreateMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .color(BRAND_BLUE)
                            .description(format!("**OCR CHECK**\n```\n{image_str}\n```")),
                    )
                    .reference_message(&msg),
            )
            .await
        {
            consume_serenity_error("OCR CHECK".into(), e);
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
