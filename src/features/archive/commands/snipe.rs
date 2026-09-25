use serenity::all::Context;

use crate::app::App;
use crate::command::Meta;
use crate::command::error::{Error, Result};
use crate::command::slash::cx::SlashCx;
use crate::command::slash::{Reply, Slash};
use crate::domain::Snowflake;
use crate::features::archive::{controls, deletion, secrets, store};
use crate::features::guildlog::attribution::Attribution;
use crate::platform::discord::partial::{PartialAttachment, PartialMessage, PartialUser};
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::Button;
use aegis_macros::{meta, slash};

#[slash]
pub struct Snipe;

impl Slash for Snipe {
    const META: Meta = meta! {
        name: "snipe",
        short: "Shows recently deleted messages",
        full: "Shows recently deleted messages. Basically a quick access to message logs. Always ephemeral.",
        category: Moderation,
        user: [MANAGE_MESSAGES],
    };

    async fn run(self, cx: &mut SlashCx) -> Result<Reply> {
        let (embed, buttons) = page(
            &cx.app,
            &cx.ctx,
            cx.guild_snowflake()?,
            cx.channel_id().get(),
            cx.author_id().get(),
            0,
        )
        .await?;

        Ok(Reply::new(embed).buttons(buttons).ephemeral())
    }
}

pub async fn page(
    app: &App,
    ctx: &Context,
    guild: Snowflake,
    channel: Snowflake,
    owner: Snowflake,
    at: u64,
) -> Result<(Embed, Vec<Button>)> {
    let total = store::count_deleted(&app.pool, guild, channel).await?;
    let at = at.min(total.saturating_sub(1));

    let Some(deleted) = store::nth_deleted(&app.pool, guild, channel, at).await? else {
        return Err(Error::empty().title("no deleted messages found"));
    };

    let key = app.secrets.of(&app.pool, ctx, guild).await?;
    let message = PartialMessage {
        id: deleted.message,
        guild_id: Some(guild),
        channel_id: channel,
        referenced_message_id: deleted.parent,
        content: secrets::open(key.as_ref(), deleted.body.as_deref()),
        author: PartialUser {
            id: deleted.author,
            name: deleted.author_name,
            display_name: deleted.author_display_name,
            avatar_url: deleted.author_avatar_url,
        },
        attachments: deleted
            .attachments
            .and_then(|stored| serde_json::from_value::<Vec<PartialAttachment>>(stored).ok())
            .unwrap_or_default(),
        created_at: deleted.created_at,
    };

    Ok((
        deletion::entry(
            &message,
            Attribution::Unknown,
            None,
            ctx.cache.current_user().id.get(),
        ),
        controls::nav(owner, at, total),
    ))
}
