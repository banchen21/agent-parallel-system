use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use crate::api::auth_utils::validate_token;
use crate::utils::log_broadcaster::get_log_sender;

#[derive(Deserialize)]
struct TokenQuery {
    token: String,
}

/// GET /logs/stream?token=<JWT>
///
/// SSE 端点，推送实时 tracing 日志流。认证通过 URL 查询参数 token（JWT Access Token）。
/// 前端使用 EventSource 订阅此端点。
#[get("/logs/stream")]
pub async fn log_stream_handler(
    _req: HttpRequest,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if validate_token(&query.token).is_err() {
        return HttpResponse::Unauthorized().body("invalid token");
    }

    let Some(tx) = get_log_sender() else {
        return HttpResponse::ServiceUnavailable().body("log broadcaster not initialized");
    };

    let rx = tx.subscribe();

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(entry) => {
                    let json = serde_json::to_string(&entry).unwrap_or_default();
                    let data = web::Bytes::from(format!("data: {}\n\n", json));
                    return Some((Ok::<_, actix_web::Error>(data), rx));
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return None,
            }
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}
