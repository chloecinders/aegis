use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::features::automod::sources;
use crate::platform::ocr;
use crate::platform::text::truncate;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct OcrCheck {}

impl Command for OcrCheck {
    const META: Meta = meta! {
        name: "ocrcheck",
        aliases: ["ocr_check"],
        short: "Runs an image through OCR",
        full: "Runs an attached or replied-to image through OCR (Optical Character Recognition). This returns text found within an image. \
        Hard to read text may not be returned correctly. This command can be used to build automod rules",
        category: Admin,
        user: [MANAGE_GUILD],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        if !ocr::available() {
            return Err(Error::empty().title("instance built without ocr features"));
        }

        let source = cx
            .msg
            .attachments
            .iter()
            .chain(
                cx.msg
                    .referenced_message
                    .iter()
                    .flat_map(|replied| replied.attachments.iter()),
            )
            .find(|attachment| {
                attachment.content_type.as_deref().is_some_and(|kind| {
                    [
                        "image/png",
                        "image/jpeg",
                        "image/webp",
                        "image/gif",
                        "image/bmp",
                    ]
                    .iter()
                    .any(|readable| kind.starts_with(readable))
                })
            });

        let Some(attachment) = source else {
            return Err(Error::new(cx.input())
                .title("no image found")
                .with_all("attach or reply to an image"));
        };

        let bytes = sources::shrunk(attachment)
            .fetch(&cx.app.http)
            .await
            .map_err(|_| Error::empty().title("image unreadable"))?;

        let Some(reading) = ocr::read(&bytes).await else {
            return Err(Error::empty().title("image unreadable"));
        };

        Ok(Response::embed(
            Embed::new("OCR RESULT")
                .subtitle(format!("File: `{}`", attachment.filename))
                .quote(match reading.text.trim().is_empty() {
                    true => String::from("no text found"),
                    false => truncate::clamp(&reading.text, 1500),
                })
                .tone(Tone::Info),
        ))
    }
}
