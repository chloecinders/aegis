use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::features::automod::enforce::{Enforced, enforce};
use crate::features::automod::eval::Hit;
use crate::features::automod::images::Images;
use crate::features::automod::rule::{self, Rule, Source};
use crate::features::automod::subject::{Fixed, record_of, wielded};
use crate::features::automod::{cache, eval, sources};
use crate::platform::discord::dispatch::MessageCx;
use crate::platform::ocr;
use crate::platform::text::fuzzy::Haystack;

pub async fn screen(cx: &MessageCx) -> Result<()> {
    let guild = cx.msg.guild_id.map(|guild| guild.get()).unwrap_or_default();
    let enabled = cx.app.rules.enabled(&cx.app.pool, guild).await?;

    if enabled.is_empty() {
        return Ok(());
    }

    let counts = sources::counts(&cx.msg, sources::needs(&enabled));
    let roles: Vec<Snowflake> = cx
        .msg
        .member
        .as_ref()
        .map(|member| member.roles.iter().map(|role| role.get()).collect())
        .unwrap_or_default();
    let age = rule::account_age(*cx.msg.author.id.created_at());
    let record = record_of(&cx.app, enabled.iter(), guild, cx.msg.author.id.get()).await?;
    let permissions = wielded(&cx.ctx, enabled.iter(), guild, cx.msg.author.id, &roles).await?;
    let wanted = cache::wanted(&enabled);
    let mut hits: Vec<Hit> = Vec::new();

    let fixed = Fixed {
        channel: cx.msg.channel_id.get(),
        roles: &roles,
        permissions,
        age,
        counts,
        record: record.as_ref(),
    };

    for source in wanted.iter().filter(|source| !source.is_expensive()) {
        let Some(text) = sources::text(&cx.msg, *source) else {
            continue;
        };

        collect(
            &enabled,
            &fixed.observed(*source, Haystack::new(&text)),
            &mut hits,
        );
    }

    let mut enforced = Enforced::default();

    enforce(cx, &enabled, &hits, &mut enforced).await;

    if !ocr::available() || !worth_reading(&enabled, &hits) {
        return Ok(());
    }

    let Some(mut images) = Images::open(cx, &enabled) else {
        return Ok(());
    };

    while let Some(text) = images.next().await {
        collect(
            &enabled,
            &fixed.observed(Source::Image, Haystack::new(&text)),
            &mut hits,
        );

        enforce(cx, &enabled, &hits, &mut enforced).await;

        if !worth_reading(&enabled, &hits) {
            break;
        }
    }

    Ok(())
}

fn worth_reading(enabled: &[Rule], hits: &[Hit]) -> bool {
    enabled
        .iter()
        .filter(|rule| rule.body.has_source(Source::Image))
        .any(|rule| !hits.iter().any(|hit| hit.rule == rule.id))
}

fn collect(rules: &[Rule], observed: &eval::Observed, hits: &mut Vec<Hit>) {
    for rule in rules {
        if hits.iter().any(|hit| hit.rule == rule.id) {
            continue;
        }

        if let Ok(hit) = eval::evaluate(rule, observed) {
            hits.push(hit);
        }
    }
}
