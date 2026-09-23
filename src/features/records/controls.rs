use serde_json::Value;
use serenity::all::{
    ButtonStyle, CreateInputText, CreateLabel, CreateModal, CreateModalComponent, InputTextStyle,
    Permissions,
};

use crate::app::App;
use crate::command::Boxed;
use crate::command::edit::Change;
use crate::command::error::{Error, Result};
use crate::domain::Snowflake;
use crate::domain::action::{Action, Amendment};
use crate::domain::ids::ActionId;
use crate::domain::reason::{Note, Reason};
use crate::features::punishments::store as punishments;
use crate::features::records::{amend, refreshed, store, ui};
use crate::features::references::{self, Captured};
use crate::platform::discord::interact::{
    Click, Control, Custom, Reaction, Router, Strangers, stale,
};
use crate::platform::text::duration;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::Button;

fn parts(action: &ActionId, back: Option<u32>) -> Vec<String> {
    let mut parts = vec![action.as_str().to_string()];

    if let Some(page) = back {
        parts.push(page.to_string());
    }

    parts
}

pub fn attached(
    owner: Snowflake,
    action: &ActionId,
    reference: Option<&Captured>,
    note: Option<&Note>,
) -> Vec<Button> {
    let mut buttons = Vec::new();

    if reference.is_some()
        && let Some(control) = references::controls::button(owner, action)
    {
        buttons.push(control);
    }

    if note.is_some()
        && let Some(id) = Custom::new("log-note", owner, [action.as_str().to_string()]).render()
    {
        buttons.push(Button::new(id, "View Note", ButtonStyle::Secondary));
    }

    buttons
}

pub fn nav(owner: Snowflake, target: Snowflake, page: u32, pages: u32) -> Vec<Button> {
    if pages <= 1 {
        return Vec::new();
    }

    let steps: [(&str, u32, String, bool); 5] = [
        ("first", 1, String::from("<<"), page == 1),
        (
            "prev",
            page.saturating_sub(1).max(1),
            String::from("<"),
            page == 1,
        ),
        ("at", page, format!("{page}/{pages}"), true),
        (
            "next",
            (page + 1).min(pages),
            String::from(">"),
            page == pages,
        ),
        ("last", pages, String::from(">>"), page == pages),
    ];

    steps
        .into_iter()
        .filter_map(|(name, to, label, off)| {
            let id = Custom::new(
                "log-page",
                owner,
                [name.to_string(), to.to_string(), target.to_string()],
            )
            .render()?;

            Some(Button::new(id, label, ButtonStyle::Secondary).disabled(off))
        })
        .collect()
}

pub fn browse(
    owner: Snowflake,
    actions: &[Action],
    page: u32,
    pages: u32,
    target: Snowflake,
) -> Vec<Button> {
    let mut out = nav(owner, target, page, pages);

    out.extend(actions.iter().enumerate().filter_map(|(index, action)| {
        let id = Custom::new(
            "log-open",
            owner,
            [action.id.as_str().to_string(), page.to_string()],
        )
        .render()?;

        Some(Button::new(
            id,
            (index + 1).to_string(),
            ButtonStyle::Secondary,
        ))
    }));

    out
}

pub async fn panel(
    app: &App,
    owner: Snowflake,
    action: &Action,
    back: Option<u32>,
) -> Result<(Embed, Vec<Button>)> {
    let reference = references::store::load(&app.pool, action.guild, &action.id).await?;
    let mut buttons = Vec::new();

    if let Some(page) = back
        && let Some(id) = Custom::new(
            "log-page",
            owner,
            [
                String::from("back"),
                page.to_string(),
                action.target.to_string(),
            ],
        )
        .render()
    {
        buttons.push(Button::new(id, "Back", ButtonStyle::Secondary));
    }

    buttons.extend(attached(
        owner,
        &action.id,
        reference.as_ref(),
        action.note.as_ref(),
    ));

    if action.verb.has_duration()
        && action.state.active()
        && let Some(id) = Custom::new("log-duration", owner, parts(&action.id, back)).render()
    {
        buttons.push(Button::new(id, "Set Duration", ButtonStyle::Secondary));
    }

    if let Some(id) = Custom::new("log-reason", owner, parts(&action.id, back)).render() {
        buttons.push(Button::new(id, "Set Reason", ButtonStyle::Secondary));
    }

    if let Some(id) = Custom::new("log-delete", owner, [action.id.as_str()]).render() {
        buttons.push(Button::new(id, "Delete", ButtonStyle::Danger));
    }

    Ok((ui::record(action, reference.as_ref()), buttons))
}

fn turn(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(target)) = (
            click.guild(),
            click.part(2).and_then(|raw| raw.parse::<Snowflake>().ok()),
        ) else {
            return Ok(Reaction::private(stale()));
        };

        let total = punishments::record_count(&click.app.pool, guild, target).await?;
        let pages = (total.max(1) as u32).div_ceil(5);
        let page = click
            .part(1)
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(1)
            .clamp(1, pages);

        let actions = store::history(&click.app.pool, guild, target, page as i64).await?;
        let attached = references::store::attached(&click.app.pool, guild, &actions).await?;

        Ok(Reaction::replace(
            ui::history(target, &actions, &attached, page, pages, total),
            browse(click.owner(), &actions, page, pages, target),
        ))
    })
}

fn open(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let back = click.part(1).and_then(|raw| raw.parse::<u32>().ok());
        let found = store::load(&click.app.pool, guild, &ActionId::from(id.to_string())).await?;

        let Some(action) = found else {
            return Err(Error::bare().title("log not found"));
        };

        let (embed, buttons) = panel(&click.app, click.owner(), &action, back).await?;

        Ok(Reaction::replace(embed, buttons))
    })
}

fn show(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let action = ActionId::from(id.to_string());
        let found = store::load(&click.app.pool, guild, &action).await?;

        let Some(note) = found.and_then(|action| action.note) else {
            return Err(Error::bare().title("note not found"));
        };

        Ok(Reaction::private(ui::note(&action, &note)))
    })
}

fn ask_duration(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let record = ActionId::from(id.to_string());
        let back = click.part(1).and_then(|raw| raw.parse::<u32>().ok());

        let Some(action) = store::load(&click.app.pool, guild, &record).await? else {
            return Err(Error::bare().title("log not found"));
        };

        if !action.verb.has_duration() {
            return Err(Error::bare().title("only bans and mutes have durations"));
        }

        if !action.state.active() {
            return Err(Error::bare().title("action no longer active"));
        }

        let Some(custom) =
            Custom::new("log-duration-set", click.owner(), parts(&record, back)).render()
        else {
            return Ok(Reaction::private(stale()));
        };

        let field = CreateInputText::new(InputTextStyle::Short, "duration")
            .value(duration::compact(action.duration()))
            .placeholder("15m");

        Ok(Reaction::Open(Box::new(
            CreateModal::new(custom, "Set Duration").components(vec![CreateModalComponent::Label(
                CreateLabel::input_text("duration", field),
            )]),
        )))
    })
}

fn set_duration(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let record = ActionId::from(id.to_string());
        let back = click.part(1).and_then(|raw| raw.parse::<u32>().ok());

        let Some(action) = store::load(&click.app.pool, guild, &record).await? else {
            return Err(Error::bare().title("log not found"));
        };

        if !action.state.active() {
            return Err(Error::bare().title("action no longer active"));
        }

        let Some(window) = click.chosen().first().and_then(|raw| duration::parse(raw)) else {
            return Err(Error::bare().title("unreadable duration"));
        };

        let change = Change {
            field: "duration",
            policy: Amendment::Duration,
            before: Value::from(action.duration().num_seconds()),
            after: Value::from(window.num_seconds()),
        };

        let updated = amend::write(
            &click.app,
            &click.ctx,
            &action,
            std::slice::from_ref(&change),
        )
        .await?;

        refreshed(&click.app.pool, &click.ctx, &updated).await?;

        let (embed, buttons) = panel(&click.app, click.owner(), &updated, back).await?;

        Ok(Reaction::replace(embed, buttons))
    })
}

fn ask_reason(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let record = ActionId::from(id.to_string());
        let back = click.part(1).and_then(|raw| raw.parse::<u32>().ok());

        let Some(action) = store::load(&click.app.pool, guild, &record).await? else {
            return Err(Error::bare().title("log not found"));
        };

        let Some(custom) =
            Custom::new("log-reason-set", click.owner(), parts(&record, back)).render()
        else {
            return Ok(Reaction::private(stale()));
        };

        let field = CreateInputText::new(InputTextStyle::Paragraph, "reason")
            .value(action.reason.to_string())
            .max_length(500);

        Ok(Reaction::Open(Box::new(
            CreateModal::new(custom, "Set Reason").components(vec![CreateModalComponent::Label(
                CreateLabel::input_text("reason", field),
            )]),
        )))
    })
}

fn set_reason(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let record = ActionId::from(id.to_string());
        let back = click.part(1).and_then(|raw| raw.parse::<u32>().ok());

        let Some(action) = store::load(&click.app.pool, guild, &record).await? else {
            return Err(Error::bare().title("log not found"));
        };

        let reason = Reason::new(click.chosen().first().unwrap_or(&""));

        store::set_reason(&click.app.pool, guild, &record, &reason).await?;

        let updated = Action { reason, ..action };

        refreshed(&click.app.pool, &click.ctx, &updated).await?;

        let (embed, buttons) = panel(&click.app, click.owner(), &updated, back).await?;

        Ok(Reaction::replace(embed, buttons))
    })
}

fn delete_record(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let record = ActionId::from(id.to_string());

        let Some(action) = store::load(&click.app.pool, guild, &record).await? else {
            return Err(Error::bare().title("log not found"));
        };

        store::delete(&click.app.pool, guild, &record).await?;

        Ok(Reaction::replace(ui::deleted(&action), Vec::new()))
    })
}

pub fn register(router: &mut Router) {
    router.add(Control {
        key: "log-page",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Fork,
        handle: turn,
    });

    router.add(Control {
        key: "log-open",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Fork,
        handle: open,
    });

    router.add(Control {
        key: "log-note",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Fork,
        handle: show,
    });

    router.add(Control {
        key: "log-duration",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Deny,
        handle: ask_duration,
    });

    router.add(Control {
        key: "log-duration-set",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Deny,
        handle: set_duration,
    });

    router.add(Control {
        key: "log-reason",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Deny,
        handle: ask_reason,
    });

    router.add(Control {
        key: "log-reason-set",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Deny,
        handle: set_reason,
    });

    router.add(Control {
        key: "log-delete",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Deny,
        handle: delete_record,
    });
}
