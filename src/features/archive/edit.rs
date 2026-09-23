use serenity::all::{CreateAttachment, GuildId, Message};

use crate::app::App;
use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::archive::{reply_line, store};
use crate::features::guildlog;
use crate::platform::discord::partial::PartialMessage;
use crate::platform::text::diff;
use crate::platform::ui::embed::{self, Embed, codeblock, mention};
use crate::platform::ui::tone::Tone;

pub struct Logged {
    pub embed: Embed,
    pub diff: Option<String>,
}

pub fn entry(before: &PartialMessage, after: &str, guild: Snowflake) -> Logged {
    let (was, now) = (body(&before.content), body(after));
    let attach = was.len() > 500 || now.len() > 500;

    let embed = Embed::new("MESSAGE EDITED")
        .subtitle(format!(
            "ID: `{}` [jump]({})",
            before.id,
            embed::jump(guild, before.channel_id, before.id)
        ))
        .subtitle(format!(
            "Author: {}",
            mention(before.author.id, Some(&before.author.name))
        ))
        .subtitle(format!("Channel: <#{}>", before.channel_id))
        .maybe_lead(reply_line(before))
        .tone(Tone::Warn);

    match attach {
        true => Logged {
            embed,
            diff: Some(diff::create(&was, &now)),
        },
        false => Logged {
            embed: embed.body(format!(
                "Before:\n{}\nAfter:\n{}",
                codeblock(&was),
                codeblock(&now)
            )),
            diff: None,
        },
    }
}

fn body(content: &str) -> String {
    match content.trim().is_empty() {
        true => String::from("(no text)"),
        false => String::from(content),
    }
}

pub async fn record(
    app: &App,
    ctx: &serenity::all::Context,
    guild: GuildId,
    after: &Message,
) -> Result<()> {
    let Some(before) = app.recent.peek(after.channel_id.get(), after.id.get()) else {
        return Ok(());
    };

    if before.content == after.content {
        return Ok(());
    }

    let stored = app
        .secrets
        .protect(&app.pool, ctx, guild.get(), &after.content)
        .await?;

    store::revise(
        &app.pool,
        &store::Revision {
            message: after.id.get(),
            body: stored,
            at: chrono::Utc::now(),
        },
    )
    .await?;

    let entry = entry(&before, &after.content, guild.get());
    let logged = guildlog::attaching(
        app,
        ctx,
        guild.get(),
        LogType::MessageUpdate,
        &entry.embed,
        guildlog::Subject {
            target: before.author.id,
            moderator: None,
            action: None,
        },
        entry
            .diff
            .map(|diff| CreateAttachment::bytes(diff.into_bytes(), "msg.diff")),
        &[],
    )
    .await;

    let mut revised = PartialMessage::clone(&before);

    revised.content = after.content.to_string();

    app.recent.remember(std::sync::Arc::new(revised));

    logged.map(|_| ())
}
