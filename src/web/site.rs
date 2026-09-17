use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use crate::web::{Shared, assets};

include!(concat!(env!("OUT_DIR"), "/site.rs"));

struct Page {
    name: &'static str,
    body: &'static [u8],
}

fn at(name: &str) -> Option<Page> {
    let found = FILES
        .binary_search_by(|(route, _)| (*route).cmp(name))
        .ok()?;
    let (name, body) = FILES[found];

    Some(Page { name, body })
}

fn resolve(path: &str, exists: impl Fn(&str) -> bool) -> Option<String> {
    let trimmed = path.trim_start_matches('/').trim_end_matches('/');

    if trimmed.contains('\\') || trimmed.split('/').any(|part| part == "..") {
        return None;
    }

    if trimmed.is_empty() {
        return exists("index.html").then(|| String::from("index.html"));
    }

    [
        String::from(trimmed),
        format!("{trimmed}.html"),
        format!("{trimmed}/index.html"),
    ]
    .into_iter()
    .find(|name| exists(name))
}

fn find(path: &str) -> Option<Page> {
    at(&resolve(path, |name| at(name).is_some())?)
}

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub async fn root(State(web): State<Shared>, uri: Uri) -> Response {
    let framed = uri.query().is_some_and(|query| {
        query
            .split('&')
            .any(|pair| pair.split('=').next() == Some("frame_id"))
    });

    match framed {
        true => assets::dashboard(State(web)).await.into_response(),
        false => page(uri).await,
    }
}

pub async fn page(uri: Uri) -> Response {
    let Some(page) = find(uri.path()) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let caching = match page.name.ends_with(".html") {
        true => "no-cache",
        false => "public, max-age=604800",
    };

    (
        [
            (header::CONTENT_TYPE, mime(page.name)),
            (header::CACHE_CONTROL, caching),
        ],
        page.body,
    )
        .into_response()
}
