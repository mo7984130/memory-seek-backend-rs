use axum::{extract::Request, middleware::Next, response::Response};
use tracing::Instrument;

pub async fn tracing_span(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    let request = Request::from_parts(parts, body);

    let span = tracing::info_span!(
        "request",
        method = %method,
        uri = %uri,
        client_ip = tracing::field::Empty,
        trace_id = tracing::field::Empty
    );
    next.run(request).instrument(span).await
}
