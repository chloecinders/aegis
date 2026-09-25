use serenity::all::{CreateAllowedMentions, CreateAttachment, CreateMessage, GenericChannelId};

use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::MessageId;
use crate::platform::text::lexer;
use aegis_macros::{command, meta};

#[command]
pub struct MsgDbg {
    #[arg]
    channel: Option<GenericChannelId>,
    #[arg(reply)]
    message: Arg<MessageId>,
}

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
        let target = match (
            self.message.was_inferred(),
            cx.msg.referenced_message.clone(),
        ) {
            (true, Some(replied)) => *replied,
            _ => {
                let channel = self.channel.unwrap_or_else(|| cx.channel_id());

                channel
                    .message(
                        &cx.ctx,
                        serenity::all::MessageId::new(self.message.into_value().get()),
                    )
                    .await
                    .map_err(|_| {
                        Error::new(cx.input())
                            .title("no message found")
                            .with_all("not found in this channel")
                    })?
            }
        };

        let tokens: Vec<String> = lexer::lex(&target.content)
            .into_iter()
            .map(|token| token.raw)
            .collect();

        let attached = CreateAttachment::bytes(
            format!("lexed: {tokens:?}\n\n{target:#?}").into_bytes(),
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
