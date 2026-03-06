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
    models::chat::{CreateChatSessionRequest, SendChatMessageRequest},
};

/// 创建聊天会话
pub async fn create_chat_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateChatSessionRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let session = state.chat_service.create_session(request).await?;
    Ok(success_response(json!(session), "聊天会话创建成功"))
}

/// 获取用户的全局聊天会话
pub async fn get_user_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_user_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let session = state.chat_service.get_or_create_global_session(channel_user_id).await?;
    Ok(success_response(json!(session), "会话获取成功"))
}

/// 获取聊天会话详情
pub async fn get_chat_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let session = state
        .chat_service
        .get_session(session_id)
        .await?
        .ok_or_else(|| AppError::NotFound("聊天会话不存在".to_string()))?;
    
    Ok(success_response(json!(session), "会话详情获取成功"))
}

/// 获取会话消息历史
pub async fn get_session_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    let messages = state.chat_service.get_session_messages(session_id, 50).await?;
    Ok(success_response(json!(messages), "消息历史获取成功"))
}

/// 发送聊天消息
pub async fn send_chat_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SendChatMessageRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    // 获取或创建会话
    let session = state
        .chat_service
        .get_or_create_global_session(request.channel_user_id)
        .await?;
    
    // 保存用户消息
    let user_message = state
        .chat_service
        .add_message(session.id, "user", &request.content, None)
        .await?;
    
    // 调用 LLM 获取响应
    let response_text = format!("Echo: {}", request.content);
    
    // 保存助手响应
    let assistant_message = state
        .chat_service
        .add_message(session.id, "assistant", &response_text, None)
        .await?;
    
    Ok(success_response(
        json!({
            "user_message": user_message,
            "assistant_message": assistant_message
        }),
        "消息发送成功"
    ))
}

/// 关闭聊天会话
pub async fn close_chat_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    
    // 标记会话为非活跃
    state.chat_service.close_session(session_id).await?;
    
    Ok(success_response(json!({}), "会话已关闭"))
}
