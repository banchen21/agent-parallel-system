use actix_web::http::header::AUTHORIZATION;
use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    error::ErrorUnauthorized,
};
use futures_util::future::LocalBoxFuture;
use std::future::{Ready, ready};
use tracing::debug;

// 引入你的工具类
use crate::api::auth_utils::validate_token;

pub struct Auth;

impl<S, B> Transform<S, ServiceRequest> for Auth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddleware { service }))
    }
}

pub struct AuthMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 1. 提取 Header (注意：直接 unwrap 会在没有 Header 时导致进程崩溃，建议安全处理)
        let auth_header = req.headers().get(AUTHORIZATION);

        if auth_header.is_none() {
            return Box::pin(async move { Err(ErrorUnauthorized("Missing Authorization Header")) });
        }

        let auth_str = auth_header.unwrap().to_str().unwrap_or("");

        // 简单的格式校验
        if !auth_str.starts_with("Bearer ") || auth_str.len() < 8 {
            return Box::pin(async move { Err(ErrorUnauthorized("Invalid Token Format")) });
        }
        let token = auth_str.replace("Bearer ", "");

        // 2. 验证 Token
        match validate_token(&token) {
            Ok(c) => {
                if c.token_type != "access" {
                    return Box::pin(async move {
                        Err(ErrorUnauthorized("Token is not an Access Token"))
                    });
                }

                // --- 验证成功逻辑 ---
                // 将用户信息存入 extensions 供后续逻辑使用
                req.extensions_mut().insert(c.sub.clone());

                // 调用下一个服务 (真正的逻辑)
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                })
            }
            Err(e) => {
                // --- 验证失败逻辑 ---
                Box::pin(async move { Err(ErrorUnauthorized(format!("Invalid Token: {}", e))) })
            }
        }
    }
}
