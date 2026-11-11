use serenity::all::{CreateAttachment, GuildId, Message};

use crate::app::App;
use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::archive::{images, reply_line, store};
use crate::features::guildlog;
use crate::platform::discord::partial::{PartialAttachment, PartialMessage};
use crate::platform::text::diff;
use crate::platform::ui::embed::{self, Embed, codeblock, mention};
use crate::platform::ui::tone::Tone;

pub struct Logged {
    pub embed: Embed,
    pub diff: Option<String>,
}

pub fn entry(before: &PartialMessage, after: &str, guild: Snowflake, unavailable: usize) -> Logged {
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
        .maybe_subtitle((unavailable > 0).then(|| format!("Images unavailable: {unavailable}")))
        .maybe_lead(reply_line(before))
        .tone(Tone::Warn);

    if before.content == after {
        return Logged { embed, diff: None };
    }

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

    let remaining = PartialMessage::from(after).attachments;
    let removed: Vec<PartialAttachment> = before
        .attachments
        .iter()
        .filter(|attachment| {
            !remaining
                .iter()
                .any(|present| present.url.split('?').next() == attachment.url.split('?').next())
        })
        .cloned()
        .collect();

    if before.content == after.content && removed.is_empty() {
        return Ok(());
    }

    if before.content != after.content {
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
    }

    let mut recovered = images::recover(app, &removed).await;

    images::restore(app, ctx, guild.get(), before.id, &mut recovered).await?;

    let entry = entry(
        &before,
        &after.content,
        guild.get(),
        recovered.unavailable.len(),
    );
    let mut files = recovered.files();

    files.truncate(9);
    files.extend(
        entry
            .diff
            .map(|diff| CreateAttachment::bytes(diff.into_bytes(), "msg.diff")),
    );

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
        files,
        &[],
    )
    .await;

    let mut revised = PartialMessage::clone(&before);

    revised.content = after.content.to_string();
    revised.attachments = remaining;

    app.recent.remember(std::sync::Arc::new(revised));

    logged.map(|_| ())
}
