use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use aegis_macros::{command, meta};

#[command]
pub struct ColonThree {}

impl Command for ColonThree {
    const META: Meta = meta! {
        name: ":3",
        short: "",
        full: ":3",
        category: Misc,
        edit: Rerun,
        hidden: true
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let sent = cx
            .channel_id()
            .send_message(
                &cx.ctx.http,
                serenity::all::CreateMessage::new()
                    .content(":3")
                    .reference_message(&*cx.msg)
                    .allowed_mentions(
                        serenity::all::CreateAllowedMentions::new().replied_user(false),
                    ),
            )
            .await;

        Ok(match sent {
            Ok(message) => Response::Sent(message.id),
            Err(_) => Response::None,
        })
    }
}
