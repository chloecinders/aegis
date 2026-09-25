use serenity::all::{MessageId, User};

use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::Snowflake;
use crate::domain::ids::ActionId;
use crate::features::punishments::store as punishments;
use crate::features::records::{controls, store, ui};
use crate::features::references;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::{self, Button};
use aegis_macros::{command, meta};

#[command]
pub struct Log {
    #[arg(reply)]
    target: Arg<User>,
    #[arg]
    record: Option<ActionId>,
    #[flag(short = 'p', desc = "Which page of the log to read")]
    page: Option<u32>,
}

impl Command for Log {
    const META: Meta = meta! {
        name: "log",
        short: "Shows the moderation history of a member",
        full: "Lists every recorded action against a member, long reasons may be truncated. Opens a log in full if a log ID is provided.",
        category: Records,
        one_of: [MODERATE_MEMBERS, KICK_MEMBERS, BAN_MEMBERS, MANAGE_NICKNAMES],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let target = self.target.into_value().id.get();

        if let Some(id) = self.record {
            return opened(cx, guild, target, &id).await;
        }

        let total = punishments::record_count(cx.pool(), guild, target).await?;

        let pages = (total.max(1) as u32).div_ceil(5);
        let page = self.page.unwrap_or(1).clamp(1, pages);

        let actions = store::history(cx.pool(), guild, target, page as i64).await?;
        let attached = references::store::attached(cx.pool(), guild, &actions).await?;
        let listing = ui::history(target, &actions, &attached, page, pages, total);
        let buttons = controls::browse(cx.author_id().get(), &actions, page, pages, target);

        post(cx, &listing, &buttons).await.map(Response::Sent)
    }
}

async fn opened(cx: &Cx, guild: Snowflake, target: Snowflake, id: &ActionId) -> Result<Response> {
    let found = store::load(cx.pool(), guild, id).await?;

    let Some(action) = found.filter(|action| action.target == target) else {
        return Err(Error::empty().title("log not found"));
    };

    let (embed, buttons) = controls::panel(&cx.app, cx.author_id().get(), &action, None).await?;

    post(cx, &embed, &buttons).await.map(Response::Sent)
}

async fn post(cx: &Cx, embed: &Embed, buttons: &[Button]) -> Result<MessageId> {
    cx.present(
        embed,
        buttons.chunks(5).take(5).map(reply::row).collect(),
        "post the member log",
    )
    .await
}
