use serenity::all::{CreateAllowedMentions, CreateMessage};

use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use aegis_macros::{command, meta};

#[command]
pub struct Say {
    #[arg(rest)]
    message: String,
}

impl Command for Say {
    const META: Meta = meta! {
        name: "say",
        short: "🐈",
        full: "🐈",
        category: Developer,
        developer: true,
        edit: Fixed,
        hidden: true,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let mut posted = CreateMessage::new()
            .content(&self.message)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Some(replied) = &cx.msg.referenced_message {
            posted = posted.reference_message(&**replied);
        }

        let sent = cx.channel_id().send_message(&cx.ctx.http, posted).await;

        cx.app
            .pending
            .expect_deletion(cx.channel_id().get(), cx.msg.id.get());

        let _ = cx.msg.delete(&cx.ctx.http, None).await;

        Ok(match sent {
            Ok(message) => Response::Sent(message.id),
            Err(_) => Response::None,
        })
    }
}
