use std::sync::Arc;

use serenity::all::ChannelId;

use crate::command::cx::Cx;
use crate::domain::Snowflake;
use crate::domain::ids::RuleId;
use crate::domain::logtype::LogType;
use crate::features::archive;
use crate::features::automod::eval::Hit;
use crate::features::automod::rule::{Notify, Rule};
use crate::features::automod::{eval, origin, ui};
use crate::features::guildlog::{self, Posted};
use crate::features::punishments::executor::{self, Reply, Subject};
use crate::platform::discord::dispatch::MessageCx;
use crate::platform::discord::fetch;
use crate::platform::ui::reply;

struct Acted {
    severity: u8,
    announced: Option<Posted>,
}

#[derive(Default)]
pub struct Enforced {
    acted: Option<Acted>,
    asked: Vec<(RuleId, bool)>,
    deleted: bool,
}

pub async fn enforce(cx: &MessageCx, enabled: &[Rule], hits: &[Hit], enforced: &mut Enforced) {
    let guild = cx.msg.guild_id.map(|guild| guild.get()).unwrap_or_default();
    let bot = cx.ctx.cache.current_user().id.get();
    let target = cx.msg.author.id.get();

    let Some((hit, rule)) = eval::severest(hits, enabled) else {
        return;
    };

    let severity = rule.body.outcome.severity();

    if enforced
        .acted
        .as_ref()
        .is_some_and(|acted| severity <= acted.severity)
    {
        return;
    }

    if !ready(cx, enforced, rule, target) {
        return;
    }

    let repeat = !cx.app.punished.ready((rule.id.clone(), rule.guild, target));
    let mut announced = None;

    if !repeat {
        if let Some(punishment) = eval::punishment(&rule.body.outcome, guild, bot, target) {
            let member = match cx.msg.guild_id {
                Some(id) => fetch::member(&cx.ctx, id, cx.msg.author.id).await.ok(),
                None => None,
            };

            let subject = match member {
                Some(member) => Subject::Present(Box::new(member)),
                None => Subject::Absent(Box::new(cx.msg.author.clone())),
            };

            let mut invocation = Cx::new(Arc::clone(&cx.app), cx.ctx.clone(), Arc::clone(&cx.msg));

            if let Err(failure) = executor::apply(
                &mut invocation,
                punishment,
                subject,
                Reply::None,
                None,
                false,
            )
            .await
            {
                cx.app.reporter.note(
                    "automod could not punish",
                    format!(
                        "{}: {}; {}",
                        rule.name,
                        failure.headline(),
                        origin(cx).describe()
                    ),
                );

                return;
            }
        }

        let entry = ui::triggered(hit, target, Some(&cx.msg.author.name));

        announced = match enforced.acted.as_ref().and_then(|acted| acted.announced) {
            Some(written) => {
                if let Err(failure) = guildlog::rewrite(&cx.ctx, written, &entry).await {
                    cx.app.reporter.record(&failure, origin(cx));
                }

                Some(written)
            }
            None => announce(cx, guild, bot, target, &entry, rule.body.outcome.notify).await,
        };
    }

    if rule.body.outcome.delete && !enforced.deleted {
        enforced.deleted = true;

        let noted = archive::store::removed(
            &cx.app.pool,
            guild,
            cx.msg.id.get(),
            archive::store::Removal::Automod,
            Some(&rule.name),
        )
        .await;

        if let Err(failure) = noted {
            cx.app.reporter.record(&failure, origin(cx));
        }

        let _ = cx.msg.delete(&cx.ctx.http, None).await;
    }

    enforced.acted = Some(Acted {
        severity,
        announced,
    });
}

async fn announce(
    cx: &MessageCx,
    guild: Snowflake,
    bot: Snowflake,
    target: Snowflake,
    entry: &crate::platform::ui::embed::Embed,
    notify: Notify,
) -> Option<Posted> {
    match notify {
        Notify::Log => logged(cx, guild, bot, target, entry).await,
        Notify::None => None,
        Notify::Channel(channel) => {
            let channel = ChannelId::new(channel);
            let sent = channel
                .widen()
                .send_message(&cx.ctx.http, reply::plain(entry))
                .await
                .ok()?;

            Some(Posted {
                channel,
                message: sent.id,
            })
        }
    }
}

async fn logged(
    cx: &MessageCx,
    guild: Snowflake,
    bot: Snowflake,
    target: Snowflake,
    entry: &crate::platform::ui::embed::Embed,
) -> Option<Posted> {
    match guildlog::post(
        &cx.app,
        &cx.ctx,
        guild,
        LogType::MemberModeration,
        entry,
        guildlog::Subject {
            target,
            moderator: Some(bot),
            action: None,
        },
    )
    .await
    {
        Ok(posted) => posted,
        Err(failure) => {
            cx.app.reporter.record(&failure, origin(cx));

            None
        }
    }
}

fn ready(cx: &MessageCx, enforced: &mut Enforced, rule: &Rule, member: Snowflake) -> bool {
    if let Some((_, answer)) = enforced.asked.iter().find(|(id, _)| *id == rule.id) {
        return *answer;
    }

    let answer = match rule.body.after {
        None => true,
        Some(threshold) => {
            cx.app
                .strikes
                .record(&rule.id, rule.guild, member, threshold.window)
                >= threshold.count
        }
    };

    enforced.asked.push((rule.id.clone(), answer));

    answer
}
