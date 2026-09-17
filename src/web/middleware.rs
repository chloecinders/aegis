use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

pub async fn framing(request: Request, next: Next) -> Response {
    let mut answer = next.run(request).await;

    answer.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("frame-ancestors 'self' https://discord.com https://*.discord.com https://*.discordsays.com"),
    );

    answer
}
