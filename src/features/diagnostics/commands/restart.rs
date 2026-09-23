use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use aegis_macros::{command, meta};
use serenity::all::CreateMessage;

#[command]
pub struct Restart {}

impl Command for Restart {
    const META: Meta = meta! {
        name: "restart",
        short: "Restarts the bot",
        full: "The supervisor is expected to restart the bot. If you are a developer make sure that something like systemd is starting it again.",
        category: Developer,
        developer: true,
        edit: Fixed,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let sent = cx
            .channel_id()
            .send_message(&cx.ctx.http, CreateMessage::new().content("Restarting!"))
            .await;

        cx.app.stopping.ask();

        Ok(match sent {
            Ok(message) => Response::Sent(message.id),
            Err(_) => Response::None,
        })
    }
}
