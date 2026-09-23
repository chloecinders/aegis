use serenity::all::{CreateAllowedMentions, CreateAttachment, CreateMessage};

use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::platform::text::lexer;
use aegis_macros::{command, meta};

#[command]
pub struct MsgDbg {}

impl Command for MsgDbg {
    const META: Meta = meta! {
        name: "msgdbg",
        short: "Dumps the message object",
        full: "Dumps the message object.",
        category: Developer,
        developer: true,
        edit: Fixed,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let Some(replied) = cx.msg.referenced_message.clone() else {
            return Err(Error::new(cx.input())
                .title("no message to read")
                .with_all("reply to a message to use this"));
        };

        let tokens: Vec<String> = lexer::lex(&replied.content)
            .into_iter()
            .map(|token| token.raw)
            .collect();

        let attached = CreateAttachment::bytes(
            format!("lexed: {tokens:?}\n\n{replied:#?}").into_bytes(),
            "message.txt",
        );

        let sent = cx
            .channel_id()
            .send_message(
                &cx.ctx.http,
                CreateMessage::new()
                    .add_file(attached)
                    .reference_message(&*cx.msg)
                    .allowed_mentions(CreateAllowedMentions::new().replied_user(false)),
            )
            .await;

        Ok(match sent {
            Ok(message) => Response::Sent(message.id),
            Err(_) => Response::None,
        })
    }
}
