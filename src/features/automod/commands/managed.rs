use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::Snowflake;
use crate::features::automod::clause;
use crate::features::automod::commands::rule::{clauses, detail};
use crate::features::automod::managed::{self, Managed, Offer};
use crate::features::automod::rule::Mode;
use crate::platform::text::truncate;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::{self, Button};
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

pub const RESERVED: [&str; 15] = [
    "list",
    "help",
    "show",
    "clauses",
    "clause",
    "add",
    "subscribe",
    "remove",
    "unsubscribe",
    "mode",
    "respond",
    "write",
    "description",
    "publish",
    "delete",
];

pub const USAGE: [(&str, &str); 8] = [
    ("list", "list managed rules"),
    ("show <name>", "shows the description of a rule"),
    ("clauses", "the full list of response clauses"),
    ("clauses <clause>", "the description of a specific clause"),
    ("add <name>", "subscribes to a rule"),
    ("remove <name>", "unsubscribe from a rule"),
    ("mode <name> <active|disabled>", "change the mode of a rule"),
    ("respond <name>", "write/edit the response to a trigger"),
];

pub const DEVELOPER_USAGE: [(&str, &str); 4] = [
    ("write <name>", "writes/edits a rule"),
    ("description <name> <text>", "sets a rule description"),
    (
        "publish <name> <active|disabled>",
        "changes the visibility of a rule",
    ),
    ("delete <name>", "deletes a rule"),
];

#[command]
pub struct ManagedRules {
    #[arg]
    action: Option<String>,
    #[arg]
    subject: Option<String>,
    #[arg(rest)]
    rest: Option<String>,
}

impl Command for ManagedRules {
    const META: Meta = meta! {
        name: "managed",
        aliases: ["managedrules", "managed_rules"],
        short: "Subscribes to developer managed rules",
        full: "Managed rules are written by the developers of the bot. \
        The exact clauses/conditions of these rules are purposefully hidden \
        to avoid circumvention by malicious actors. \
        Developer managed rules may contain rules like anti-spam bot rules, \
        which are annoying to manage and are useful to a lot of servers. \
        Server administrators can choose the responses to these rules, as in what \
        is done when they are triggered. Run `/p/managed help` for subcommands.",
        category: Admin,
        user: [MANAGE_GUILD],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;

        let Some(action) = self.action.as_deref() else {
            return list(cx, guild).await;
        };

        let action = action.to_lowercase();

        match action.as_str() {
            "list" => list(cx, guild).await,
            "help" => Ok(Response::embed(usage(cx))),
            "show" => show(cx, guild, self.subject.as_deref()).await,
            "clauses" | "clause" => match self.subject.as_deref().map(str::to_lowercase) {
                Some(name) => Ok(Response::embed(detail(
                    clause::CLAUSES
                        .iter()
                        .find(|found| {
                            found.keyword == name && clause::Part::Response.allows(found.keyword)
                        })
                        .ok_or_else(|| Error::empty().title("clause not found"))?,
                ))),
                None => Ok(Response::embed(managed::ui::clauses())),
            },
            "add" | "subscribe" => add(cx, guild, self.subject.as_deref()).await,
            "remove" | "unsubscribe" => remove(cx, guild, self.subject.as_deref()).await,
            "mode" => mode(cx, guild, self.subject.as_deref(), self.rest.as_deref()).await,
            "respond" => respond(cx, guild, self.subject.as_deref()).await,
            "write" => write(cx, self.subject.as_deref()).await,
            "description" => description(cx, self.subject.as_deref(), self.rest.as_deref()).await,
            "publish" => {
                let managed = authored(cx, self.subject.as_deref()).await?;
                let lowered = self.rest.as_deref().map(|raw| raw.trim().to_lowercase());

                let Some(mode) = lowered.as_deref().and_then(Mode::parse) else {
                    return Err(Error::empty().title("expected active or disabled"));
                };

                managed::store::set_mode(cx.pool(), &managed.id, mode).await?;
                cx.app.rules.forget_everywhere();

                Ok(Response::embed(managed::ui::inspect(&Managed {
                    mode,
                    ..managed
                })))
            }
            "delete" => delete_managed(cx, self.subject.as_deref()).await,
            _ => show(cx, guild, Some(&action)).await,
        }
    }
}

fn developer(cx: &Cx) -> Result<()> {
    match cx.app.is_developer(cx.author_id().get()) {
        true => Ok(()),
        false => Err(Error::empty().title("👽")),
    }
}

async fn offered(cx: &Cx, guild: Snowflake, name: Option<&str>) -> Result<Offer> {
    let name = name.ok_or_else(|| Error::empty().title("provide the managed rule"))?;

    let managed = managed::store::find(cx.pool(), name)
        .await?
        .ok_or_else(|| Error::empty().title("managed rule not found"))?;
    let subscription = managed::store::subscription(cx.pool(), guild, &managed.id).await?;

    if managed.mode != Mode::Active && subscription.is_none() {
        return Err(Error::empty().title("managed rule not found"));
    }

    Ok(Offer {
        managed,
        subscription,
    })
}

async fn authored(cx: &Cx, name: Option<&str>) -> Result<Managed> {
    developer(cx)?;

    let name = name.ok_or_else(|| Error::empty().title("provide the managed rule"))?;

    managed::store::find(cx.pool(), name)
        .await?
        .ok_or_else(|| Error::empty().title("managed rule not found"))
}

async fn posted(cx: &Cx, embed: Embed, buttons: Vec<Button>) -> Result<Response> {
    cx.present(
        &embed,
        buttons.chunks(5).take(5).map(reply::row).collect(),
        "post managed rule",
    )
    .await
    .map(Response::Sent)
}

async fn list(cx: &Cx, guild: Snowflake) -> Result<Response> {
    let offers = managed::store::offers(cx.pool(), guild).await?;

    posted(
        cx,
        managed::ui::listing(&offers, 0),
        managed::controls::browse(cx.author_id().get(), &offers, 0),
    )
    .await
}

async fn show(cx: &Cx, guild: Snowflake, name: Option<&str>) -> Result<Response> {
    if cx.app.is_developer(cx.author_id().get())
        && let Some(name) = name
        && let Some(managed) = managed::store::find(cx.pool(), name).await?
    {
        return Ok(Response::embed(managed::ui::inspect(&managed)));
    }

    let offer = offered(cx, guild, name).await?;

    posted(
        cx,
        managed::ui::offer(&offer),
        managed::controls::all(cx.author_id().get(), &offer, None),
    )
    .await
}

async fn add(cx: &Cx, guild: Snowflake, name: Option<&str>) -> Result<Response> {
    let mut offer = offered(cx, guild, name).await?;

    if offer.managed.mode != Mode::Active {
        return Err(Error::empty().title("rule not published"));
    }

    if !managed::store::subscribe(cx.pool(), guild, &offer.managed.id).await? {
        return Err(Error::empty().title("already subscribed to rule"));
    }

    cx.app.rules.forget(guild);

    offer.subscription = managed::store::subscription(cx.pool(), guild, &offer.managed.id).await?;

    posted(
        cx,
        managed::ui::offer(&offer),
        managed::controls::all(cx.author_id().get(), &offer, None),
    )
    .await
}

async fn remove(cx: &Cx, guild: Snowflake, name: Option<&str>) -> Result<Response> {
    let offer = offered(cx, guild, name).await?;

    if !managed::store::unsubscribe(cx.pool(), guild, &offer.managed.id).await? {
        return Err(Error::empty().title("server not subscribed to rule"));
    }

    cx.app.rules.forget(guild);

    Ok(Response::embed(
        Embed::new("RULE UNSUBSCRIBED")
            .subtitle(format!("Name: {}", offer.managed.name))
            .tone(Tone::Danger),
    ))
}

async fn mode(
    cx: &Cx,
    guild: Snowflake,
    name: Option<&str>,
    raw: Option<&str>,
) -> Result<Response> {
    let mut offer = offered(cx, guild, name).await?;

    if offer.subscription.is_none() {
        return Err(Error::empty().title("server not subscribed to rule"));
    }

    let lowered = raw.map(|raw| raw.trim().to_lowercase());

    let Some(mode) = lowered.as_deref().and_then(Mode::parse) else {
        return Err(Error::empty().title("expected active or disabled"));
    };

    managed::store::set_guild_mode(cx.pool(), guild, &offer.managed.id, mode).await?;
    cx.app.rules.forget(guild);

    offer.subscription = managed::store::subscription(cx.pool(), guild, &offer.managed.id).await?;

    posted(
        cx,
        managed::ui::offer(&offer),
        managed::controls::all(cx.author_id().get(), &offer, None),
    )
    .await
}

async fn respond(cx: &Cx, guild: Snowflake, name: Option<&str>) -> Result<Response> {
    let mut offer = offered(cx, guild, name).await?;

    if offer.subscription.is_none() {
        return Err(Error::empty().title("server not subscribed to rule"));
    }

    let Some((block, offset)) = clauses(cx.input()) else {
        return Err(Error::new(cx.input())
            .title("no response provided")
            .with_all("write the clauses below"));
    };

    let response = clause::parse_as(cx.input(), offset, clause::Part::Response)?;

    managed::store::set_response(cx.pool(), guild, &offer.managed.id, block, &response).await?;
    cx.app.rules.forget(guild);

    offer.subscription = managed::store::subscription(cx.pool(), guild, &offer.managed.id).await?;

    posted(
        cx,
        managed::ui::offer(&offer),
        managed::controls::all(cx.author_id().get(), &offer, None),
    )
    .await
}

async fn write(cx: &Cx, name: Option<&str>) -> Result<Response> {
    developer(cx)?;

    let name = name.ok_or_else(|| Error::empty().title("provide the managed rule"))?;

    if RESERVED.contains(&name.to_lowercase().as_str()) {
        return Err(Error::empty().title("name is reserved"));
    }

    let Some((block, offset)) = clauses(cx.input()) else {
        return Err(Error::new(cx.input())
            .title("no clauses provided")
            .with_all("write the clauses below"));
    };

    let body = clause::parse_as(cx.input(), offset, clause::Part::Detection)?;
    let existing = managed::store::find(cx.pool(), name).await?;
    let managed = Managed {
        id: existing
            .as_ref()
            .map(|found| found.id.clone())
            .unwrap_or_else(managed::generate),
        name: existing
            .as_ref()
            .map(|found| found.name.clone())
            .unwrap_or_else(|| name.to_string()),
        description: existing
            .as_ref()
            .map(|found| found.description.clone())
            .unwrap_or_default(),
        mode: existing.as_ref().map_or(Mode::Disabled, |found| found.mode),
        source: block.to_string(),
        body,
    };

    managed::store::save(cx.pool(), &managed).await?;
    cx.app.rules.forget_everywhere();

    Ok(Response::embed(managed::ui::saved(
        &managed,
        existing.is_some(),
    )))
}

async fn description(cx: &Cx, name: Option<&str>, raw: Option<&str>) -> Result<Response> {
    let managed = authored(cx, name).await?;
    let written = raw.map(str::trim).unwrap_or_default();

    if written.is_empty() {
        return Err(Error::empty().title("no description provided"));
    }

    let trimmed = truncate::clamp(written, 300);

    managed::store::set_description(cx.pool(), &managed.id, &trimmed).await?;

    let description_of = Managed {
        description: trimmed,
        ..managed
    };

    Ok(Response::embed(managed::ui::inspect(&description_of)))
}

async fn delete_managed(cx: &Cx, name: Option<&str>) -> Result<Response> {
    let managed = authored(cx, name).await?;

    managed::store::delete(cx.pool(), &managed.name).await?;
    cx.app.rules.forget_everywhere();

    Ok(Response::embed(
        Embed::new("MANAGED RULE DELETED")
            .subtitle(format!("Name: {}", managed.name))
            .tone(Tone::Danger),
    ))
}

fn usage(cx: &Cx) -> Embed {
    let mut listed: Vec<String> = USAGE
        .iter()
        .map(|(form, description)| format!("`managed {form}` - {description}"))
        .collect();

    if cx.app.is_developer(cx.author_id().get()) {
        listed.push(String::new());
        listed.push(String::from("**developer**"));
        listed.extend(
            DEVELOPER_USAGE
                .iter()
                .map(|(form, description)| format!("`managed {form}` - {description}")),
        );
    }

    Embed::new("MANAGED RULE COMMANDS")
        .body(listed.join("\n"))
        .tone(Tone::Info)
}
