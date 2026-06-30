use serenity::{
    all::{Context, CreateAttachment, CreateEmbed, CreateMessage, Message},
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
    utils::is_developer,
};
use aegis_macros::command;

pub struct OcrDbg;

impl OcrDbg {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for OcrDbg {
    fn get_name(&self) -> &'static str {
        "ocrdbg"
    }

    fn get_short(&self) -> &'static str {
        "Shows stored OCR text and rule matches for a message"
    }

    fn get_full(&self) -> &'static str {
        "Reply to a message that has an image attachment. Shows the OCR text extracted from each attachment and whether it matched any automod OCR rule. Only works for messages processed after the bot last started."
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
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        handler: &Handler,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if !is_developer(&msg.author) {
            return Ok(());
        }

        let Some(reply) = msg.referenced_message.as_ref() else {
            return Err(CommandError {
                title: String::from("You must reply to a message with image attachments."),
                hint: Some(String::from(
                    "Only messages processed after the last bot restart have stored OCR data.",
                )),
                arg: None,
            });
        };

        let reply_id = reply.id.get();

        trace.point("fetching_ocr_cache");

        let entries = {
            let cache = handler.ocr_result_cache.lock().await;
            cache.get(reply_id).cloned()
        };

        let Some(entries) = entries else {
            return Err(CommandError {
                title: String::from("No OCR data found for that message."),
                hint: Some(String::from(
                    "OCR data is only stored for messages with image attachments that were processed after the last bot restart. The message may also have been evicted from the cache.",
                )),
                arg: None,
            });
        };

        if entries.is_empty() {
            return Err(CommandError {
                title: String::from("No OCR data found for that message."),
                hint: None,
                arg: None,
            });
        }

        let mut output = String::new();

        for (i, entry) in entries.iter().enumerate() {
            if entries.len() > 1 {
                output.push_str(&format!("**Attachment {}**\n", i + 1));
            }

            let text_display = if entry.text.is_empty() {
                String::from("*(no text extracted)*")
            } else {
                format!("```\n{}\n```", entry.text)
            };

            output.push_str(&format!("**OCR Text:**\n{text_display}\n"));

            match &entry.matched {
                Some((rule_name, rule_id, pattern, is_regex)) => {
                    let pattern_type = if *is_regex { "regex" } else { "fuzzy" };
                    output.push_str(&format!(
                        "**Matched Rule:** `{rule_name}` (ID: `{rule_id}`)\n**Pattern ({pattern_type}):** `{pattern}`\n✅ **Would have triggered automod**\n"
                    ));
                }
                None => {
                    output.push_str(
                        "**Matched Rule:** *(none)*\n❌ **Did not match any OCR rule**\n",
                    );
                }
            }

            if i + 1 < entries.len() {
                output.push('\n');
            }
        }

        if output.len() > 3500 {
            let r = CreateMessage::new()
                .add_file(CreateAttachment::bytes(output.into_bytes(), "ocrdbg.txt"))
                .reference_message(&msg);

            if let Err(e) = msg.channel_id.send_message(&ctx, r).await {
                crate::utils::consume_serenity_error(String::from("OCRDBG RUN"), e);
            }
        } else {
            let r = CreateMessage::new()
                .add_embed(CreateEmbed::new().color(BRAND_BLUE).description(output))
                .reference_message(&msg);

            if let Err(e) = msg.channel_id.send_message(&ctx, r).await {
                crate::utils::consume_serenity_error(String::from("OCRDBG RUN"), e);
            }
        }

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
            silence_typing: true,
        }
    }
}
