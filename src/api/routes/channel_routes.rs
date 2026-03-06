use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    api::routes::{success_response, extract_user_id, AppState},
    core::errors::AppError,
    models::channel::{CreateChannelConfigRequest, UpdateChannelConfigRequest},
};

/// 创建通道配置
pub async fn create_channel_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateChannelConfigRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let config = state.channel_service.create_channel_config(request).await?;
    Ok(success_response(json!(config), "通道配置创建成功"))
}

/// 获取所有活跃通道
pub async fn get_active_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let channels = state.channel_service.get_active_channels().await?;
    Ok(success_response(json!(channels), "通道列表获取成功"))
}

/// 获取通道配置详情
pub async fn get_channel_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(config_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let config = state
        .channel_service
        .get_channel_config(config_id)
        .await?
        .ok_or_else(|| AppError::NotFound("通道配置不存在".to_string()))?;
    
    Ok(success_response(json!(config), "通道配置获取成功"))
}

/// 更新通道配置
pub async fn update_channel_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(config_id): Path<Uuid>,
    Json(request): Json<UpdateChannelConfigRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let config = state
        .channel_service
        .update_channel_config(config_id, request)
        .await?;
    
    Ok(success_response(json!(config), "通道配置更新成功"))
}

/// 获取通道用户信息
pub async fn get_channel_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let user = state
        .channel_service
        .get_channel_user(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("通道用户不存在".to_string()))?;
    
    Ok(success_response(json!(user), "通道用户获取成功"))
}

/// 绑定通道用户到系统用户
pub async fn bind_channel_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((channel_user_id, system_user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    state
        .channel_service
        .bind_channel_user(channel_user_id, system_user_id)
        .await?;
    
    Ok(success_response(json!({}), "用户绑定成功"))
}

/// Webhook 端点 - 接收来自 Telegram 的消息
pub async fn telegram_webhook(
    State(_state): State<AppState>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    // 这里处理 Telegram webhook 消息
    // 实现细节取决于具体的 Telegram 集成方式
    
    Ok(success_response(json!({}), "Webhook 已接收"))
}

/// Webhook 端点 - 接收来自 Discord 的消息
pub async fn discord_webhook(
    State(_state): State<AppState>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    // 这里处理 Discord webhook 消息
    
    Ok(success_response(json!({}), "Webhook 已接收"))
}
