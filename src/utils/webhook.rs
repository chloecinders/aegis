#![allow(unused_variables)] // while we still have the huge match

use chrono::Local;
use serenity::{
    Error as SerenityError,
    all::{CreateEmbed, CreateMessage, ModelError},
    json,
};
use sqlx::Error as SqlxError;
use tracing::warn;

use crate::{BOT_CONFIG, constants::BRAND_RED};

pub fn send_error(title: String, body: String) {
    let Some(webhook_url) = BOT_CONFIG.webhook.clone() else {
        return;
    };

    let time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    tokio::spawn(async move {
        let msg = CreateMessage::new().embed(
            CreateEmbed::new()
                .color(BRAND_RED)
                .description(format!("**{title}**\n`{time}` {body}")),
        );

        let body = json::to_string(&msg);
        let err = reqwest::Client::new()
            .post(webhook_url)
            .body(body.unwrap_or_default())
            .header("content-type", "application/json")
            .send()
            .await;

        match err {
            Ok(body) => warn!(
                "Sent error; response = {}",
                body.text().await.unwrap_or_default()
            ),
            Err(e) => warn!("Error while sending error... {e}"),
        }
    });
}

pub fn consume_serenity_error(action: String, err: SerenityError) {
    let Some(webhook_url) = BOT_CONFIG.webhook.clone() else {
        return;
    };

    let time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // This will be updated as time goes on and we get more error reports
    let body = match &err {
        SerenityError::Decode(_, value) => String::from("UNHANDLED: DECODE"),
        SerenityError::Format(error) => String::from("UNHANDLED: FORMAT"),
        SerenityError::Io(error) => String::from("UNHANDLED: IO"),
        SerenityError::Json(error) => String::from("UNHANDLED: JSON"),
        SerenityError::Model(error) => match error {
            ModelError::BulkDeleteAmount => String::from("UNHANDLED"),
            ModelError::DeleteMessageDaysAmount(_) => String::from("UNHANDLED"),
            ModelError::EmbedAmount => String::from("UNHANDLED"),
            ModelError::EmbedTooLarge(_) => String::from("UNHANDLED"),
            ModelError::GuildNotFound => String::from("UNHANDLED"),
            ModelError::RoleNotFound => String::from("UNHANDLED"),
            ModelError::MemberNotFound => String::from("UNHANDLED"),
            ModelError::ChannelNotFound => String::from("UNHANDLED"),
            ModelError::MessageAlreadyCrossposted => String::from("UNHANDLED"),
            ModelError::CannotCrosspostMessage => String::from("UNHANDLED"),
            ModelError::Hierarchy => String::from("UNHANDLED"),
            ModelError::InvalidPermissions { required, present } => format!(
                "Not enough permissions; required {}; present {}",
                required.bits(),
                present.bits()
            ),
            ModelError::InvalidUser => String::from("UNHANDLED"),
            ModelError::ItemMissing => String::from("UNHANDLED"),
            ModelError::WrongGuild => String::from("UNHANDLED"),
            ModelError::MessageTooLong(_) => String::from("UNHANDLED"),
            ModelError::MessagingBot => String::from("UNHANDLED"),
            ModelError::InvalidChannelType => String::from("UNHANDLED"),
            ModelError::NameTooShort => String::from("UNHANDLED"),
            ModelError::NameTooLong => String::from("UNHANDLED"),
            ModelError::NotAuthor => String::from("UNHANDLED"),
            ModelError::NoTokenSet => String::from("UNHANDLED"),
            ModelError::DeleteNitroSticker => String::from("UNHANDLED"),
            ModelError::NoStickerFileSet => String::from("UNHANDLED"),
            ModelError::StickerAmount => String::from("UNHANDLED"),
            ModelError::CannotEditVoiceMessage => String::from("UNHANDLED"),
            _ => String::from("UNHANDLED"),
        },
        SerenityError::ExceededLimit(_, _) => String::from("UNHANDLED: EXCEEDEDLIMIT"),
        SerenityError::NotInRange(_, _, _, _) => String::from("UNHANDLED: NOTINRANGE"),
        SerenityError::Other(_) => String::from("UNHANDLED: OTHER"),
        SerenityError::Url(_) => String::from("UNHANDLED: URL"),
        SerenityError::Client(error) => String::from("UNHANDLED: CLIENT"),
        SerenityError::Gateway(error) => String::from("UNHANDLED: GATEWAY"),
        SerenityError::Http(http_error) => String::from("UNHANDLED: HTTP"),
        SerenityError::Tungstenite(error) => String::from("UNHANDLED: TUNGSTENITE"),
        _ => String::from("UNHANDLED: OTHER"),
    };

    warn!("Encountered Error: {action}; {body}; {err}");

    tokio::spawn(async move {
        let msg = CreateMessage::new().embed(CreateEmbed::new().color(BRAND_RED).description(
            format!("**SERENITY ERROR: {action}**\n`{time}` {body}\nOriginal: {err}"),
        ));

        let body = json::to_string(&msg);
        let err = reqwest::Client::new()
            .post(webhook_url)
            .body(body.unwrap_or_default())
            .header("content-type", "application/json")
            .send()
            .await;

        match err {
            Ok(body) => warn!(
                "Sent error; response = {}",
                body.text().await.unwrap_or(String::from("(None)"))
            ),
            Err(e) => warn!("Error while sending error... {e}"),
        }
    });
}

pub fn consume_pgsql_error(action: String, err: SqlxError) {
    let Some(webhook_url) = BOT_CONFIG.webhook.clone() else {
        return;
    };

    let time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // This will be updated as time goes on and we get more error reports
    let body = match &err {
        SqlxError::Configuration(error) => String::from("UNHANDLED PGSQL"),
        SqlxError::InvalidArgument(_) => String::from("UNHANDLED PGSQL"),
        SqlxError::Database(database_error) => String::from("UNHANDLED PGSQL"),
        SqlxError::Io(error) => String::from("UNHANDLED PGSQL"),
        SqlxError::Tls(error) => String::from("UNHANDLED PGSQL"),
        SqlxError::Protocol(_) => String::from("UNHANDLED PGSQL"),
        SqlxError::RowNotFound => String::from("UNHANDLED PGSQL"),
        SqlxError::TypeNotFound { type_name } => String::from("UNHANDLED PGSQL"),
        SqlxError::ColumnIndexOutOfBounds { index, len } => String::from("UNHANDLED PGSQL"),
        SqlxError::ColumnNotFound(_) => String::from("UNHANDLED PGSQL"),
        SqlxError::ColumnDecode { index, source } => String::from("UNHANDLED PGSQL"),
        SqlxError::Encode(error) => String::from("UNHANDLED PGSQL"),
        SqlxError::Decode(error) => String::from("UNHANDLED PGSQL"),
        SqlxError::AnyDriverError(error) => String::from("UNHANDLED PGSQL"),
        SqlxError::PoolTimedOut => String::from("UNHANDLED PGSQL"),
        SqlxError::PoolClosed => String::from("UNHANDLED PGSQL"),
        SqlxError::WorkerCrashed => String::from("UNHANDLED PGSQL"),
        SqlxError::Migrate(migrate_error) => String::from("UNHANDLED PGSQL"),
        SqlxError::InvalidSavePointStatement => String::from("UNHANDLED PGSQL"),
        SqlxError::BeginFailed => String::from("UNHANDLED PGSQL"),
        _ => String::from("UNHANDLED PGSQL"),
    };

    warn!("Encountered Error: {action}; {body}; {err}");

    tokio::spawn(async move {
        let msg = CreateMessage::new().embed(CreateEmbed::new().color(BRAND_RED).description(
            format!("**PGSQL ERROR: {action}**\n`{time}` {body}\nOriginal: {err}"),
        ));

        let body = json::to_string(&msg);
        let err = reqwest::Client::new()
            .post(webhook_url)
            .body(body.unwrap_or_default())
            .header("content-type", "application/json")
            .send()
            .await;

        match err {
            Ok(body) => warn!(
                "Sent error; response = {}",
                body.text().await.unwrap_or(String::from("(None)"))
            ),
            Err(e) => warn!("Error while sending error... {e}"),
        }
    });
}
