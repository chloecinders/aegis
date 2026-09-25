use serenity::all::Permissions;

use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response, permissions};
use crate::domain::Snowflake;
use crate::features::snippets::store::Snippet;
use crate::features::snippets::{Scope, nameable, store, ui};
use crate::platform::text::lexer::{Span, Token, lex};
use crate::platform::ui::embed::Embed;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

pub const USAGE: [(&str, &str); 5] = [
    ("show <name>", "shows the command of a snippet"),
    ("add <name> <command>", "creates a new user snippet"),
    ("delete <name>", "removes a user snippet"),
    (
        "server add <name> <command>",
        "creates a server snippet, requires MANAGE_SERVER",
    ),
    ("server delete <name>", "removes a server snippet"),
];

#[command]
pub struct Snippets {
    #[arg]
    action: Option<String>,
    #[arg(rest)]
    tail: Option<String>,
}

impl Command for Snippets {
    const META: Meta = meta! {
        name: "snippet",
        aliases: ["snippets", "alias", "aliases"],
        short: "Allows for custom aliases for commands",
        full: "Snippets allow you to define custom alises for commands. \
        To create one use `/p/snippet add (name) (command)`. If you want to \
        for example create a snippet that softbans scam bots you could do: \
        `/p/snippet add scam softban scam bot +c 7`. The snippet would then \
        be usable with `/p//p/scam` and basically run `/p/softban scam bot +c \
        7`. Server administrators can define server-wide snippets, usable by \
        all members (given they have the required permissions for the command \
        in the snippet using) `/p/snippet server add`.\n\nRun `/p/snippet help` \
        for more information on subcommands.",
        category: Utilities,
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let tokens = lex(cx.input());
        let action = self.action.as_deref().map(str::to_lowercase);
        let tail = self.tail.as_deref();

        if action.as_deref() == Some("server") {
            if !cx.has(Permissions::MANAGE_GUILD).await? {
                return Err(Error::new(cx.input())
                    .title("missing required permissions")
                    .with_all("missing permissions"));
            }

            let (action, tail) = head(tail);

            return scoped(cx, Scope::Server(guild), action, tail, &tokens, 2).await;
        }

        match action.as_deref() {
            None | Some("list") => list(cx, guild).await,
            Some("help") => Ok(usage(cx.app.prefix())),
            Some("show") => show(cx, guild, tail, &tokens).await,
            other => {
                scoped(
                    cx,
                    Scope::User(cx.author_id().get()),
                    other,
                    tail,
                    &tokens,
                    1,
                )
                .await
            }
        }
    }
}

async fn scoped(
    cx: &Cx,
    scope: Scope,
    action: Option<&str>,
    tail: Option<&str>,
    tokens: &[Token],
    at: usize,
) -> Result<Response> {
    match action {
        Some("add") | Some("set") => save(cx, scope, tail, tokens, at + 1).await,
        Some("delete") | Some("remove") => remove(cx, scope, tail, tokens, at + 1).await,
        _ => Err(Error::new(cx.input())
            .title("expected add, delete, show, server or list")
            .with_span(
                spanned(tokens, at),
                match action {
                    Some(_) => "unknown subcommand",
                    None => "missing subcommand",
                },
            )
            .with_span_help(spanned(tokens, at), "provide a valid subcommand", "add")),
    }
}

async fn list(cx: &Cx, guild: Snowflake) -> Result<Response> {
    let snippets = store::visible(cx.pool(), guild, cx.author_id().get()).await?;

    Ok(Response::embed(ui::listing(&snippets, cx.app.prefix())))
}

async fn show(cx: &Cx, guild: Snowflake, tail: Option<&str>, tokens: &[Token]) -> Result<Response> {
    let (name, _) = head(tail);

    let Some(name) = name else {
        return Err(Error::new(cx.input())
            .title("no snippet provided")
            .with_span(spanned(tokens, 2), "expected <snippet: String>")
            .with_span_help(spanned(tokens, 2), "provide the snippet", "scam"));
    };

    let found = store::resolve(cx.pool(), guild, cx.author_id().get(), name)
        .await?
        .ok_or_else(|| Error::empty().title("snippet not found"))?;

    Ok(Response::embed(ui::shown(&found, cx.app.prefix())))
}

async fn remove(
    cx: &Cx,
    scope: Scope,
    tail: Option<&str>,
    tokens: &[Token],
    at: usize,
) -> Result<Response> {
    let (name, _) = head(tail);

    let Some(name) = name else {
        return Err(Error::new(cx.input())
            .title("no snippet provided")
            .with_span(spanned(tokens, at), "expected <snippet: String>")
            .with_span_help(spanned(tokens, at), "provide the snippet", "scam"));
    };

    if !store::delete(cx.pool(), scope, name).await? {
        return Err(Error::empty().title("snippet not found"));
    }

    Ok(Response::embed(ui::deleted(name, scope)))
}

async fn save(
    cx: &Cx,
    scope: Scope,
    tail: Option<&str>,
    tokens: &[Token],
    at: usize,
) -> Result<Response> {
    let (name, body) = head(tail);

    let Some(name) = name else {
        return Err(Error::new(cx.input())
            .title("no snippet provided")
            .with_span(spanned(tokens, at), "expected <snippet: String>")
            .with_span_help(spanned(tokens, at), "provide a snippet", "scam"));
    };

    if !nameable(name) {
        let cleaned: String = name
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .take(32)
            .collect();

        return Err(Error::new(cx.input())
            .title("invalid snippet name")
            .with_span(
                spanned(tokens, at),
                "expected one word of up to 32 letters, numbers, dashes and underscores",
            )
            .with_span_help(
                spanned(tokens, at),
                "provide a valid name",
                match cleaned.is_empty() {
                    true => String::from("scam"),
                    false => cleaned,
                },
            ));
    }

    let body = body
        .map(str::trim)
        .map(|body| body.strip_prefix(cx.app.prefix()).unwrap_or(body))
        .filter(|body| !body.is_empty());

    let Some(body) = body else {
        return Err(Error::new(cx.input())
            .title("missing snippet body")
            .with_span(spanned(tokens, at + 1), "expected <command: String>")
            .with_span_help(
                spanned(tokens, at + 1),
                "provide the command it runs",
                "ban @someone",
            ));
    };

    if body.chars().count() > 512 {
        let from = spanned(tokens, at + 1);
        let written = Span {
            start: from.start,
            len: tokens
                .last()
                .map_or(0, |token| token.span.end().saturating_sub(from.start)),
            index: from.index,
            quoted: false,
        };

        return Err(Error::new(cx.input())
            .title("snippet too long")
            .with_span(written, "over 512 characters"));
    }

    let leading = body.split_whitespace().next().unwrap_or_default();

    let Some(entry) = cx
        .app
        .registry
        .find(leading)
        .filter(|entry| entry.text.is_some())
        .copied()
    else {
        return Err(Error::new(cx.input())
            .title("expected a command")
            .with_span(spanned(tokens, at + 1), "unknown command")
            .with_span_help(spanned(tokens, at + 1), "provide a valid command", "ban"));
    };

    permissions::check_member_access(cx, &entry.meta).await?;

    let body = String::from(body);
    let existing = store::find(cx.pool(), scope, name).await?;

    if existing.is_none() && store::count(cx.pool(), scope).await? >= scope.limit() {
        return Err(Error::empty().title(format!("{} snippet limit reached", scope.limit())));
    }

    store::save(cx.pool(), scope, name, &body).await?;

    let written = Snippet {
        name: String::from(name),
        body,
        scope,
    };

    Ok(Response::embed(ui::saved(
        &written,
        existing.is_some(),
        cx.app.prefix(),
    )))
}

fn spanned(tokens: &[Token], index: usize) -> Span {
    if let Some(token) = tokens.get(index) {
        return token.span;
    }

    Span {
        start: tokens.last().map_or(0, |token| token.span.end()),
        len: 0,
        index: tokens.len(),
        quoted: false,
    }
}

fn head(tail: Option<&str>) -> (Option<&str>, Option<&str>) {
    let tail = tail.map(str::trim).filter(|tail| !tail.is_empty());

    let Some(tail) = tail else {
        return (None, None);
    };

    match tail.find(char::is_whitespace) {
        Some(at) => (Some(&tail[..at]), Some(tail[at..].trim_start())),
        None => (Some(tail), None),
    }
}

fn usage(prefix: &str) -> Response {
    let listed: Vec<String> = USAGE
        .iter()
        .map(|(form, description)| match form.is_empty() {
            true => format!("`{prefix}snippet` - {description}"),
            false => format!("`{prefix}snippet {form}` - {description}"),
        })
        .collect();

    Response::embed(
        Embed::new("SNIPPET COMMANDS")
            .body(listed.join("\n"))
            .footnote(format!(
                "A snippet is run with the prefix twice: {prefix}{prefix}scam in reply to \
                 someone runs whatever scam was written as, with anything you type after it \
                 added to the end. Names are up to 32 characters"
            ))
            .tone(Tone::Info),
    )
}
