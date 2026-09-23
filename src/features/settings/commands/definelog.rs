use serenity::all::{ChannelId, GuildChannel};

use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::features::settings::{controls, store, ui};
use aegis_macros::{command, meta};

#[command]
pub struct DefineLog {
    #[arg]
    channel: Option<GuildChannel>,
}

impl Command for DefineLog {
    const META: Meta = meta! {
        name: "definelog",
        aliases: ["dlog"],
        short: "Define channels for event logging",
        full: "Opens a menu to define event logs for the current channel. You can also pass a different channel. \
        The 'Keep' button sets the channel as a log channel for all events that currently have no channel set. \
        'All' sets the channel as the log channel for all events, overwriting existing log channels. \
        'Reset' removes the current channel from all events.",
        category: Admin,
        user: [MANAGE_GUILD],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let channel = self
            .channel
            .map_or_else(|| cx.channel_id().expect_channel(), |chosen| chosen.id);

        open(cx, guild, channel).await
    }
}

async fn open(cx: &Cx, guild: u64, channel: ChannelId) -> Result<Response> {
    let known = store::routes(cx.pool(), guild).await?;
    let embed = ui::picker();

    cx.present(
        &embed,
        controls::panel(cx.author_id().get(), &known, channel).rows(),
        "post the log picker",
    )
    .await
    .map(Response::Sent)
}
