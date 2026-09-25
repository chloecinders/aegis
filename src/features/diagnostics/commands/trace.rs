use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::MessageId;
use crate::features::diagnostics::{store, ui};
use aegis_macros::{command, meta};

#[command]
pub struct Trace {
    #[arg(reply)]
    message: Arg<MessageId>,
}

impl Command for Trace {
    const META: Meta = meta! {
        name: "trace",
        short: "Shows the trace points of a command invocation",
        full: "Shows the trace points (timings) of a command. Used for performance debugging.",
        category: Developer,
        developer: true,
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let message = self.message.into_value().get();

        let Some(run) = store::of_message(cx.pool(), message).await? else {
            return Err(Error::empty().title("no trace found for message"));
        };

        Ok(Response::embed(ui::timing(&run)))
    }
}
