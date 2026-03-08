use std::future::{ready, Ready};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use actix_web::http::header::AUTHORIZATION;

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
        // 提取 Header
        let auth_header = req.headers().get(AUTHORIZATION);

        // 使用 map_err 将 jsonwebtoken 的错误手动转换为 anyhow 错误，确保类型一致
        let auth_result = (|| -> anyhow::Result<crate::api::auth_utils::Claims> {
            let auth_val = auth_header.ok_or_else(|| anyhow::anyhow!("Missing Authorization header"))?;
            let auth_str = auth_val.to_str().map_err(|_| anyhow::anyhow!("Invalid header encoding"))?;
            
            if !auth_str.starts_with("Bearer ") {
                return Err(anyhow::anyhow!("Invalid token format"));
            }

            let token = &auth_str[7..];
            // 关键点：使用 .map_err(|e| anyhow::anyhow!(e)) 进行类型转换
            validate_token(token).map_err(|e| anyhow::anyhow!(e))
        })();

        match auth_result {
            Ok(claims) => {
                req.extensions_mut().insert(claims.sub.clone());
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                })
            }
            Err(err) => {
                let err_msg = err.to_string();
                Box::pin(async move {
                    Err(ErrorUnauthorized(format!("Unauthorized: {}", err_msg)))
                })
            }
        }
    }
}