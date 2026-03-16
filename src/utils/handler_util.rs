use actix_web::{HttpMessage, HttpRequest, HttpResponse};

/// 从 `HttpRequest` 的 extensions 中提取用户标识（String）。
/// 返回 `Ok(String)` 或 `Err(HttpResponse::Unauthorized())` 以便直接在 handler 中返回。
pub fn get_user_name(req: &HttpRequest) -> Result<String, HttpResponse> {
    match req.extensions().get::<String>() {
        Some(user) => Ok(user.clone()),
        None => Err(HttpResponse::Unauthorized().finish()),
    }
}
