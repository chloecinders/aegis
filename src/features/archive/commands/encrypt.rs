use crate::command::cx::Cx;
use crate::command::error::{Ctx, Result};
use crate::command::{Command, Meta, Response};
use crate::features::archive::store;
use crate::platform::crypto;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

#[command]
pub struct Encrypt {
    #[arg]
    state: Option<String>,
}

impl Command for Encrypt {
    const META: Meta = meta! {
        name: "encrypt",
        short: "Enables information encryption",
        full: "Generates an encryption key and posts it in this channel. The key will be used to encrypt information \
        such as message content in the bots database. Attackers compromising the database will then not be able to \
        read any message content. This should only be done in channels which administrators and the bots have \
        access to. Due to the nature of how this works deleting the key message will wipe all logged messages \
        from the database. Using this is generally recommended.",
        category: Admin,
        user: [MANAGE_GUILD],
        edit: Fixed,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;

        if self.state.as_deref() == Some("off") {
            let wiped = store::disable(cx.pool(), guild).await?;

            cx.app.secrets.forget(guild);

            return Ok(Response::embed(
                Embed::new("ENCRYPTION DISABLED")
                    .subtitle(format!("{wiped} stored messages"))
                    .tone(Tone::Danger),
            ));
        }

        let key = crypto::generate();
        let posted = cx
            .channel_id()
            .send_message(
                &cx.ctx.http,
                reply::plain(
                    &Embed::new("ENCRYPTION KEY")
                        .subtitle("Please do not delete this key. Doing so will wipe all encrypted data from the database.")
                        .quote(BASE64.encode(key))
                        .tone(Tone::Warn),
                ),
            )
            .await
            .ctx("post encryption key")?;

        let saved = store::enable(
            cx.pool(),
            guild,
            cx.channel_id().expect_channel(),
            posted.id,
        )
        .await;

        if let Err(failure) = saved {
            let _ = posted.delete(&cx.ctx.http, None).await;

            return Err(failure);
        }

        cx.app.secrets.forget(guild);

        Ok(Response::Sent(posted.id))
    }
}
