use std::path::{Path, PathBuf};

use serenity::all::{CreateAllowedMentions, CreateMessage, EditMessage};

use crate::app::updater::{self, apply, download};
use crate::command::cx::Cx;
use crate::command::error::{Ctx as _, Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::logtype::LogType;
use crate::features::settings::store;
use crate::platform::text::truncate;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct Update {
    #[flag(
        short = 'a',
        desc = "Announces the attached notes to every server that has a channel for it"
    )]
    announce: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error(transparent)]
    Fetching(#[from] download::Error),
    #[error(transparent)]
    Unpacking(#[from] apply::Failure),
    #[error("could not write the staged build: {0}")]
    Staging(std::io::Error),
    #[error("could not hand over to the staged build: {0}")]
    Handing(std::io::Error),
}

impl Failure {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Failure::Fetching(failure) => failure.hint(),
            _ => None,
        }
    }
}

pub fn stalled(failure: &Failure) -> String {
    let headline = match failure {
        Failure::Fetching(download::Error::Unfinished { .. }) => {
            String::from("Latest run failed, fix your code idiot!")
        }
        failure => failure.to_string(),
    };

    match failure.hint() {
        Some(hint) => format!("{headline}\n-# {hint}"),
        None => headline,
    }
}

pub fn announcement(notes: &str) -> Embed {
    let notes = notes.trim();

    let (title, detail) = match notes.strip_prefix("# ") {
        Some(headed) => match headed.split_once('\n') {
            Some((heading, rest)) => (heading.trim(), rest.trim()),
            None => (headed.trim(), ""),
        },
        None => ("", notes),
    };

    let heading = match title.is_empty() {
        true => String::from("AEGIS UPDATE"),
        false => format!("AEGIS UPDATE: {}", title.to_uppercase()),
    };

    let body = match detail.is_empty() {
        true => String::from("the bot has been updated"),
        false => truncate::clamp(detail, 3500),
    };

    Embed::new(heading).body(body).tone(Tone::Info)
}

async fn stage(cx: &Cx, repository: &str, staged: &Path) -> std::result::Result<String, Failure> {
    let source = download::Source::new(&cx.app.http, cx.app.config.github_token.as_deref());

    cx.trace("find_build");
    let run = source.latest(repository).await?;
    download::usable(&run)?;

    cx.trace("find_artifact");
    let artifact = source.artifact(&run).await?;

    cx.trace("download_artifact");
    let archive = source.archive(&artifact).await?;

    cx.trace("unpack_artifact");

    let binary = updater::binary_name();
    let unpacked = tokio::task::spawn_blocking(move || apply::unpack(archive, &binary))
        .await
        .map_err(|_| Failure::Unpacking(apply::Failure::Unreadable))??;

    cx.trace("write_staged_build");

    apply::stage(&unpacked, staged)
        .await
        .map_err(Failure::Staging)?;

    Ok(run.head_sha)
}

impl Command for Update {
    const META: Meta = meta! {
        name: "update",
        short: "Installs the latest update",
        full: "Installs the latest update and restarts. Optionally pushes a message to the Aegis announcement log in each server. Attach a text file for the announcement body.",
        category: Developer,
        developer: true,
        edit: Fixed,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let Some(repository) = cx.app.config.repository.clone() else {
            return Err(Error::empty().title("no `repository` set in Config.toml"));
        };

        let announced = match self.announce {
            true => {
                let Some(attachment) = cx
                    .msg
                    .attachments
                    .iter()
                    .chain(
                        cx.msg
                            .referenced_message
                            .iter()
                            .flat_map(|replied| replied.attachments.iter()),
                    )
                    .find(|attachment| {
                        attachment
                            .content_type
                            .as_deref()
                            .is_some_and(|kind| kind.starts_with("text/"))
                            || attachment.filename.ends_with(".txt")
                            || attachment.filename.ends_with(".md")
                    })
                else {
                    return Err(Error::new(cx.input())
                        .title("no body text file provided")
                        .with_all("attach a .txt or .md file"));
                };

                let bytes = cx
                    .app
                    .http
                    .bytes(&attachment.url, 64 * 1024)
                    .await
                    .map_err(|_| Error::empty().title("file unreadable"))?;

                let notes = String::from_utf8(bytes)
                    .map_err(|_| Error::empty().title("file not valid utf-8"))?;

                if notes.trim().is_empty() {
                    return Err(Error::empty().title("file empty"));
                }

                Some(announcement(&notes))
            }
            false => None,
        };

        let mut notice = cx
            .channel_id()
            .send_message(
                &cx.ctx.http,
                CreateMessage::new()
                    .content("Updating!")
                    .allowed_mentions(CreateAllowedMentions::new().replied_user(false))
                    .reference_message(cx.msg.as_ref()),
            )
            .await
            .ctx("post the update notice")?;

        let staged = PathBuf::from(updater::staged_name());

        let sha = match stage(cx, &repository, &staged).await {
            Ok(staged) => staged,
            Err(failure) => {
                let _ = notice
                    .edit(&cx.ctx, EditMessage::new().content(stalled(&failure)))
                    .await;

                return Ok(Response::Sent(notice.id));
            }
        };

        if let Some(announced) = announced {
            cx.trace("announce_update");

            let routed = store::everywhere(cx.pool(), LogType::AegisAnnouncements).await?;

            for (_, channel) in &routed {
                let _ = channel
                    .widen()
                    .send_message(&cx.ctx.http, reply::plain(&announced))
                    .await;
            }
        }

        let _ = notice
            .edit(
                &cx.ctx,
                EditMessage::new().content(format!("Updated to `{}`", &sha[..sha.len().min(7)])),
            )
            .await;

        if let Err(failure) = apply::commit(&staged) {
            let _ = notice
                .edit(
                    &cx.ctx,
                    EditMessage::new().content(stalled(&Failure::Handing(failure))),
                )
                .await;

            return Ok(Response::Sent(notice.id));
        }

        cx.app.stopping.ask();

        Ok(Response::Sent(notice.id))
    }
}
