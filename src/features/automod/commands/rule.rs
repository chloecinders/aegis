use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::RuleId;
use crate::features::automod::clause::CLAUSES;
use crate::features::automod::rule::{Author, Mode, Rule};
use crate::features::automod::{clause, controls, store, ui};
use crate::platform::ui::embed::{self, Embed};
use crate::platform::ui::reply::{self, Button};
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

pub const RESERVED: [&str; 9] = [
    "list", "show", "mode", "delete", "remove", "clauses", "clause", "examples", "help",
];

pub const USAGE: [(&str, &str); 8] = [
    ("list", "list rules in the server"),
    ("<name>", "writes/edits a rule"),
    ("show <name>", "shows the full contents of a rule"),
    (
        "mode <name> <active|disabled>",
        "changes the mode of a rule",
    ),
    ("delete <name>", "removes a rule"),
    ("clauses", "the full list of clauses"),
    ("clauses <clause>", "the description of a specific clause"),
    ("examples", "rule examples"),
];

pub const EXAMPLES: [(&str, &str); 3] = [
    (
        "nitroscam",
        "on content image\n\
        match \"free nitro\"\n\
        then ban 7d\n\
        clear 1\n\
        reason posting a nitro scam",
    ),
    (
        "mentionspam",
        "when mentions > 5\n\
        after 2 in 10m\n\
        ignore role:112233445566778899\n\
        then mute 10m\n\
        reason mass mentions",
    ),
    (
        "freshaccount",
        "on join\n\
        when account younger than 1d",
    ),
];

#[command]
pub struct Rules {
    #[arg]
    action: Option<String>,
    #[arg]
    subject: Option<String>,
    #[arg(rest)]
    sample: Option<String>,
}

impl Command for Rules {
    const META: Meta = meta! {
        name: "rule",
        aliases: ["rules", "automod"],
        short: "Creates automod rules",
        full: "Creates and manages automod rules. Automod rules are written as a series of clauses, one per line. \
        For more information on subcommands run `/p/rule help`.",
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

        if RESERVED.contains(&action.as_str()) && clauses(cx.input()).is_some() {
            return Err(Error::empty().title("name is a subcommand"));
        }

        match action.as_str() {
            "list" => list(cx, guild).await,
            "help" => Ok(Response::embed(usage(cx.app.prefix()))),
            "clauses" | "clause" => match self.subject.as_deref().map(str::to_lowercase) {
                Some(name) => Ok(Response::embed(detail(
                    CLAUSES
                        .iter()
                        .find(|clause| clause.keyword == name)
                        .ok_or_else(|| Error::empty().title("clause not found"))?,
                ))),
                None => Ok(Response::embed(all_clauses())),
            },
            "examples" => Ok(Response::embed(examples(cx.app.prefix()))),
            "show" => show(cx, guild, self.subject.as_deref()).await,
            "mode" => set(cx, guild, self.subject.as_deref(), self.sample.as_deref()).await,
            "delete" | "remove" => remove(cx, guild, self.subject.as_deref()).await,
            name => match self.subject.as_deref().and_then(Mode::parse) {
                Some(mode) => change_mode(cx, guild, name, mode).await,
                None => define(cx, guild, name).await,
            },
        }
    }
}

async fn list(cx: &Cx, guild: u64) -> Result<Response> {
    let rules = store::all(cx.pool(), guild).await?;

    posted(
        cx,
        ui::listing(&rules, 0),
        controls::browse(cx.author_id().get(), &rules, 0),
    )
    .await
}

async fn load(cx: &Cx, guild: u64, name: Option<&str>) -> Result<Rule> {
    let name = name.ok_or_else(|| Error::empty().title("provide a rule"))?;

    store::find(cx.pool(), guild, name)
        .await?
        .ok_or_else(|| Error::empty().title("rule not found"))
}

async fn show(cx: &Cx, guild: u64, name: Option<&str>) -> Result<Response> {
    let rule = load(cx, guild, name).await?;

    posted(
        cx,
        ui::show(&rule),
        controls::detail(cx.author_id().get(), &rule, None),
    )
    .await
}

async fn posted(cx: &Cx, embed: Embed, buttons: Vec<Button>) -> Result<Response> {
    cx.present(
        &embed,
        buttons.chunks(5).take(5).map(reply::row).collect(),
        "post rule",
    )
    .await
    .map(Response::Sent)
}

async fn remove(cx: &Cx, guild: u64, name: Option<&str>) -> Result<Response> {
    let name = name.ok_or_else(|| Error::empty().title("provide a rule to delete"))?;

    if !store::delete(cx.pool(), guild, name).await? {
        return Err(Error::empty().title("rule not found"));
    }

    cx.app.rules.forget(guild);

    Ok(Response::embed(
        Embed::new("RULE DELETED")
            .subtitle(format!("Name: {name}"))
            .tone(Tone::Danger),
    ))
}

async fn set(cx: &Cx, guild: u64, name: Option<&str>, raw: Option<&str>) -> Result<Response> {
    let name = name.ok_or_else(|| Error::empty().title("provide a rule to change"))?;
    let lowered = raw.map(|raw| raw.trim().to_lowercase());

    let Some(mode) = lowered.as_deref().and_then(Mode::parse) else {
        return Err(Error::empty().title("expected active or disabled"));
    };

    change_mode(cx, guild, name, mode).await
}

async fn change_mode(cx: &Cx, guild: u64, name: &str, mode: Mode) -> Result<Response> {
    let rule = load(cx, guild, Some(name)).await?;

    store::set_mode(cx.pool(), &rule.id, mode).await?;
    cx.app.rules.forget(guild);

    let modified = Rule { mode, ..rule };

    posted(
        cx,
        ui::saved(&modified, true),
        controls::detail(cx.author_id().get(), &modified, None),
    )
    .await
}

async fn define(cx: &Cx, guild: u64, name: &str) -> Result<Response> {
    let Some((block, offset)) = clauses(cx.input()) else {
        return show(cx, guild, Some(name)).await;
    };

    let body = clause::parse(cx.input(), offset)?;
    let existing = store::find(cx.pool(), guild, name).await?;
    let rule = Rule {
        id: existing
            .as_ref()
            .map(|found| found.id.clone())
            .unwrap_or_else(RuleId::generate),
        guild,
        name: existing
            .as_ref()
            .map(|found| found.name.clone())
            .unwrap_or_else(|| name.to_string()),
        mode: existing.as_ref().map_or(Mode::Disabled, |found| found.mode),
        author: Author::Guild,
        source: block.to_string(),
        body,
    };

    store::save(cx.pool(), &rule).await?;
    cx.app.rules.forget(guild);

    posted(
        cx,
        ui::saved(&rule, existing.is_some()),
        controls::detail(cx.author_id().get(), &rule, None),
    )
    .await
}

pub fn clauses(input: &str) -> Option<(&str, usize)> {
    let head = input.find('\n')? + 1;
    let block = input.get(head..)?;

    match block.trim().is_empty() {
        true => None,
        false => Some((block, head)),
    }
}

fn usage(prefix: &str) -> Embed {
    let listed: Vec<String> = USAGE
        .iter()
        .map(|(form, description)| format!("`{prefix}rule {form}` - {description}"))
        .collect();

    Embed::new("RULE COMMANDS")
        .body(listed.join("\n"))
        .footnote("Run `rule clauses` to see how to define a rule")
        .tone(Tone::Info)
}

fn all_clauses() -> Embed {
    Embed::new("RULE CLAUSES")
        .body(clause::summaries(clause::Part::Whole))
        .footnote(
            "`rule clauses <clause>` to see more info on a clause, and `rule examples` \
            shows whole examples. Lines repeat and are order-independent. Match lines \
            are OR",
        )
        .tone(Tone::Info)
}

pub fn detail(clause: &clause::Clause) -> Embed {
    let mut body = String::from(clause.full);

    if !clause.params.is_empty() {
        let values: Vec<String> = clause
            .params
            .iter()
            .map(|(value, means)| format!("`{value}` - {means}"))
            .collect();

        body.push_str(&format!("\n\n**Parameters:**\n{}", values.join("\n")));
    }

    let examples: Vec<String> = clause
        .examples
        .iter()
        .map(|line| format!("`{line}`"))
        .collect();

    body.push_str(&format!("\n\n**Examples:**\n{}", examples.join("\n")));

    Embed::new(format!("CLAUSE {}", clause.keyword.to_uppercase()))
        .body(body)
        .tone(Tone::Info)
}

fn examples(prefix: &str) -> Embed {
    let listed: Vec<String> = EXAMPLES
        .iter()
        .map(|(name, block)| {
            format!(
                "**{name}**\n{}",
                embed::codeblock(&format!("{prefix}rule {name}\n{block}"))
            )
        })
        .collect();

    Embed::new("RULE EXAMPLES")
        .body(listed.join("\n\n"))
        .footnote(
            "These rules serve as examples for the syntax and what you can do with them and should not be used as written.",
        )
        .tone(Tone::Warn)
}
