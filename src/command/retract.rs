use std::sync::Arc;

use serenity::all::{Context, GenericChannelId, MessageId};

use crate::app::App;
use crate::features::records::store;

pub async fn withdraw(app: Arc<App>, ctx: Context, channel: GenericChannelId, message: MessageId) {
    let Ok(Some(record)) = store::load_invocation(&app.pool, message.get()).await else {
        return;
    };

    let Some(response) = record.response else {
        return;
    };

    let response = MessageId::new(response);

    if channel
        .delete_message(&ctx.http, response, None)
        .await
        .is_err()
    {
        tracing::debug!("response to {} was already gone", record.command);
    }
}
