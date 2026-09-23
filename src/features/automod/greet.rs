use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::automod::eval::Observed;
use crate::features::automod::rule::{self, Source};
use crate::features::automod::subject::{record_of, wielded};
use crate::features::automod::{cache, eval, ui};
use crate::features::guildlog;
use crate::platform::discord::dispatch::MemberCx;
use crate::platform::observe::report::Origin;
use crate::platform::text::fuzzy::Haystack;

pub async fn greet(cx: &MemberCx) -> Result<()> {
    let guild = cx.guild.get();
    let enabled = cx.app.rules.enabled(&cx.app.pool, guild).await?;
    let joining = cache::reading(&enabled, Source::Join);

    if joining.is_empty() {
        return Ok(());
    }

    let roles: Vec<Snowflake> = cx
        .member
        .as_ref()
        .map(|member| member.roles.iter().map(|role| role.get()).collect())
        .unwrap_or_default();

    let target = cx.user.id.get();
    let record = record_of(&cx.app, joining.iter().copied(), guild, target).await?;
    let permissions = wielded(&cx.ctx, joining.iter().copied(), guild, cx.user.id, &roles).await?;

    let observed = Observed {
        source: Source::Join,
        read: Haystack::new(""),
        channel: 0,
        roles: &roles,
        permissions,
        age: rule::account_age(*cx.user.id.created_at()),
        record: record.as_ref(),
        ..Observed::default()
    };

    for rule in joining {
        let Ok(hit) = eval::evaluate(rule, &observed) else {
            continue;
        };

        if let Err(failure) = guildlog::post(
            &cx.app,
            &cx.ctx,
            guild,
            LogType::MemberJoinLeave,
            &ui::triggered(&hit, target, Some(&cx.user.name)),
            guildlog::Subject {
                target,
                moderator: None,
                action: None,
            },
        )
        .await
        {
            cx.app.reporter.record(
                &failure,
                Origin {
                    guild: Some(guild),
                    user: Some(target),
                    ..Origin::default()
                },
            );
        }
    }

    Ok(())
}
