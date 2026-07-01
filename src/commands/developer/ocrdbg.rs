use std::sync::Arc;

use serenity::{
    all::{Context, CreateAllowedMentions, CreateAttachment, CreateEmbed, CreateMessage, Message},
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
    transformers::Transformers,
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
        "Pass a message ID or reply to a message that had an image attachment. Shows the OCR text extracted from each attachment and whether it matched any automod OCR rule. Works even if the message has been deleted, as long as it was processed after the bot last started."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::String("message_id", true)]
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
        #[transformers::string] message_id_arg: Option<String>,
        handler: &Handler,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if !is_developer(&msg.author) {
            return Ok(());
        }

        let reply_id = if let Some(id) = message_id_arg {
            id.parse::<u64>().map_err(|_| CommandError {
                title: String::from("Invalid Message ID"),
                hint: Some(String::from("it must be a numeric ID")),
                arg: None,
            })?
        } else if let Some(reply) = msg.referenced_message.as_ref() {
            reply.id.get()
        } else {
            return Err(CommandError {
                title: String::from("You must provide a Message ID or reply to a message."),
                hint: Some(String::from(
                    "Only messages processed after the last bot restart have stored OCR data.",
                )),
                arg: None,
            });
        };

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

        let mut output = format!(
            "**OCR DEBUG**\n-# Message ID: `{reply_id}` | Attachments: {}\n",
            entries.len()
        );

        for (i, entry) in entries.iter().enumerate() {
            if entries.len() > 1 {
                output.push_str(&format!("\n**Attachment {}**\n", i + 1));
            } else {
                output.push('\n');
            }

            let text_display = if entry.text.is_empty() {
                String::from("*(no text extracted)*")
            } else {
                format!("```\n{}\n```", entry.text)
            };

            output.push_str(&format!("**OCR Text:**\n{text_display}\n"));

            match &entry.matched {
                Some((rule_name, rule_id, pattern)) => {
                    output.push_str(&format!(
                        "**Matched Rule:** `{rule_name}` (ID: `{rule_id}`)\n**Pattern:** `{pattern}`\n**Status:** Triggered automod\n"
                    ));
                }
                None => {
                    output.push_str(
                        "**Matched Rule:** *(none)*\n**Status:** Did not match any OCR rule\n",
                    );
                }
            }
        }

        if output.len() > 3500 {
            let r = CreateMessage::new()
                .add_file(CreateAttachment::bytes(output.into_bytes(), "ocrdbg.txt"))
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Err(e) = msg.channel_id.send_message(&ctx, r).await {
                crate::utils::consume_serenity_error(String::from("OCRDBG RUN"), e);
            }
        } else {
            let r = CreateMessage::new()
                .add_embed(CreateEmbed::new().color(BRAND_BLUE).description(output))
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

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
