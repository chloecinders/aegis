use std::sync::Arc;

use serenity::all::{Embed, Message};

use crate::domain::Snowflake;
use crate::features::archive::{Storable, store};
use crate::platform::discord::dispatch::MessageCx;
use crate::platform::discord::partial::PartialMessage;

const ALERT: &str = "auto_moderation_message";

struct Blocked {
    message: PartialMessage,
    rule: Option<String>,
}

fn field<'a>(embed: &'a Embed, name: &str) -> Option<&'a str> {
    embed
        .fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| field.value.as_str())
}

fn blocked(alert: &Message, guild: Snowflake) -> Option<Blocked> {
    let embed = alert
        .embeds
        .iter()
        .find(|embed| embed.kind.as_deref() == Some(ALERT))?;

    if field(embed, "flagged_message_id").is_some() {
        return None;
    }

    let content = embed.description.clone()?;
    let channel = field(embed, "channel_id")?.parse::<Snowflake>().ok()?;
    let mut message = PartialMessage::from(alert);

    message.guild_id = Some(guild);
    message.channel_id = channel;
    message.content = content.to_string();

    Some(Blocked {
        message,
        rule: field(embed, "rule_name").map(String::from),
    })
}

pub async fn record(cx: &MessageCx, guild: Snowflake) {
    let Some(caught) = blocked(&cx.msg, guild) else {
        return;
    };

    let message = Arc::new(caught.message);

    let noted = store::removed(
        &cx.app.pool,
        guild,
        message.id,
        store::Removal::Automod,
        caught.rule.as_deref(),
    )
    .await;

    if let Err(failure) = noted {
        cx.app
            .reporter
            .note("could not log a blocked message", failure.to_string());
    }

    let kept = cx
        .app
        .secrets
        .protect(&cx.app.pool, &cx.ctx, guild, &message.content)
        .await;

    match kept {
        Ok(body) => cx.app.messages.send(Storable {
            message,
            body,
            system: false,
        }),
        Err(failure) => cx
            .app
            .reporter
            .note("could not archive a blocked message", failure.to_string()),
    }
}
