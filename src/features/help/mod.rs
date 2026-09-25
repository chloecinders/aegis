pub mod controls;

use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::registry::Registry;
use crate::command::{Command, Meta, Response, help};
use crate::platform::discord::interact::Router;
use crate::platform::ui::reply;
use crate::register;
use aegis_macros::{command, meta};

#[command]
pub struct Help {
    #[arg]
    command: Option<String>,
}

impl Command for Help {
    const META: Meta = meta! {
        name: "help",
        aliases: ["h", "commands"],
        short: "Lists all commands",
        full: "Lists all commands or explains commands in detail.",
        category: Misc,
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let developer = cx.app.is_developer(cx.author_id().get());
        let prefix = cx.app.prefix();

        let Some(wanted) = self.command else {
            return listing(cx, developer).await;
        };

        let found = cx
            .app
            .registry
            .find(&wanted)
            .filter(|entry| developer || !entry.meta.developer);

        let Some((entry, text)) = found.and_then(|entry| Some((entry, entry.text?))) else {
            return Err(Error::empty().title("command not found"));
        };

        Ok(Response::embed(help::detail(entry, &text, prefix)))
    }
}

async fn listing(cx: &Cx, developer: bool) -> Result<Response> {
    let pages = help::pages(&cx.app.registry, developer);
    let sheet = help::sheet(&pages, 0, cx.app.prefix());
    let nav = controls::nav(cx.author_id().get(), 0, pages.len());

    cx.present(
        &sheet,
        nav.chunks(5).take(5).map(reply::row).collect(),
        "post the command list",
    )
    .await
    .map(Response::Sent)
}

pub fn register(registry: &mut Registry) {
    register!(registry, Help);
}

pub fn control(router: &mut Router) {
    controls::register(router);
}
