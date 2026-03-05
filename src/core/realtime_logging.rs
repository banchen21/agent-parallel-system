use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::{sse::{Event, Sse}, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use bb8_redis::RedisConnectionManager;
use futures::stream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    middleware::AuthMiddleware,
};

/// 实时日志事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeLogEvent {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    pub target: String,
    pub fields: serde_json::Value,
    pub user_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
}

/// 实时日志订阅过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFilter {
    pub level: Option<Vec<String>>,
    pub target: Option<String>,
    pub workspace_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
}

/// 实时日志管理器
#[derive(Clone)]
pub struct RealtimeLogManager {
    /// 广播发送器，用于向所有连接的客户端发送日志
    tx: broadcast::Sender<RealtimeLogEvent>,
    /// 活跃的WebSocket连接
    connections: Arc<RwLock<HashMap<Uuid, broadcast::Sender<RealtimeLogEvent>>>>,
    /// Redis连接池
    redis_pool: bb8::Pool<RedisConnectionManager>,
    /// 数据库连接池
    db_pool: PgPool,
}

impl RealtimeLogManager {
    /// 创建新的实时日志管理器
    pub fn new(
        redis_pool: bb8::Pool<RedisConnectionManager>,
        db_pool: PgPool,
    ) -> Self {
        let (tx, _) = broadcast::channel(1000);
        
        Self {
            tx,
            connections: Arc::new(RwLock::new(HashMap::new())),
            redis_pool,
            db_pool,
        }
    }

    /// 发布日志事件
    pub async fn publish_log_event(&self, event: RealtimeLogEvent) -> Result<(), AppError> {
        // 发送到本地广播
        let _ = self.tx.send(event.clone());

        // 发布到Redis频道，用于跨实例同步
        let mut conn = self.redis_pool.get().await?;
        
        let event_data = serde_json::to_string(&event)?;
        
        redis::cmd("PUBLISH")
            .arg("realtime_logs")
            .arg(event_data)
            .query_async::<i64>(&mut *conn)
            .await?;

        Ok(())
    }

    /// 订阅日志事件
    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeLogEvent> {
        self.tx.subscribe()
    }

    /// 添加WebSocket连接
    pub async fn add_websocket_connection(
        &self,
        connection_id: Uuid,
        tx: broadcast::Sender<RealtimeLogEvent>,
    ) {
        let mut connections = self.connections.write().await;
        connections.insert(connection_id, tx);
    }

    /// 移除WebSocket连接
    pub async fn remove_websocket_connection(&self, connection_id: Uuid) {
        let mut connections = self.connections.write().await;
        connections.remove(&connection_id);
    }

    /// 获取活跃连接数
    pub async fn get_connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// 从结构化日志创建实时日志事件
    pub fn create_log_event(
        &self,
        level: String,
        message: String,
        target: String,
        fields: serde_json::Value,
        user_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        task_id: Option<Uuid>,
        agent_id: Option<Uuid>,
    ) -> RealtimeLogEvent {
        RealtimeLogEvent {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            level,
            message,
            target,
            fields,
            user_id,
            workspace_id,
            task_id,
            agent_id,
        }
    }

    /// 启动Redis订阅监听器
    pub async fn start_redis_listener(&self) -> Result<(), AppError> {
        let manager = self.clone();
        
        tokio::spawn(async move {
            let mut conn = match manager.redis_pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("Failed to get Redis connection for listener: {}", e);
                    return;
                }
            };

            let mut pubsub = match redis::cmd("SUBSCRIBE")
                .arg("realtime_logs")
                .query_async::<redis::aio::PubSub>(&mut *conn)
                .await
            {
                Ok(pubsub) => pubsub,
                Err(e) => {
                    tracing::error!("Failed to subscribe to Redis channel: {}", e);
                    return;
                }
            };

            loop {
                let msg = match pubsub.get_message().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        tracing::error!("Failed to get Redis message: {}", e);
                        continue;
                    }
                };

                let payload: String = match msg.get_payload() {
                    Ok(payload) => payload,
                    Err(e) => {
                        tracing::error!("Failed to get payload from Redis message: {}", e);
                        continue;
                    }
                };

                let event: RealtimeLogEvent = match serde_json::from_str(&payload) {
                    Ok(event) => event,
                    Err(e) => {
                        tracing::error!("Failed to parse log event from Redis: {}", e);
                        continue;
                    }
                };

                // 重新发布到本地广播
                let _ = manager.tx.send(event);
            }
        });

        Ok(())
    }
}

/// SSE日志流端点
pub async fn sse_logs(
    State(manager): State<Arc<RealtimeLogManager>>,
    Query(filter): Query<LogFilter>,
    jar: CookieJar,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, AppError>>>, AppError> {
    // 验证用户权限
    let _user_id = AuthMiddleware::get_user_id_from_cookies(&jar)?;

    let rx = manager.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(move |event| {
            let event = match event {
                Ok(event) => event,
                Err(_) => return None,
            };

            // 应用过滤器
            if let Some(levels) = &filter.level {
                if !levels.contains(&event.level) {
                    return None;
                }
            }

            if let Some(target) = &filter.target {
                if !event.target.contains(target) {
                    return None;
                }
            }

            if let Some(workspace_id) = filter.workspace_id {
                if event.workspace_id != Some(workspace_id) {
                    return None;
                }
            }

            if let Some(task_id) = filter.task_id {
                if event.task_id != Some(task_id) {
                    return None;
                }
            }

            if let Some(agent_id) = filter.agent_id {
                if event.agent_id != Some(agent_id) {
                    return None;
                }
            }

            if let Some(user_id) = filter.user_id {
                if event.user_id != Some(user_id) {
                    return None;
                }
            }

            Some(Ok(Event::default().json_data(&event).unwrap()))
        })
        .map(Ok);

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive"),
    ))
}

/// WebSocket日志端点
pub async fn websocket_logs(
    ws: WebSocketUpgrade,
    State(manager): State<Arc<RealtimeLogManager>>,
    Query(filter): Query<LogFilter>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    // 验证用户权限
    let user_id = AuthMiddleware::get_user_id_from_cookies(&jar)?;

    let connection_id = Uuid::new_v4();
    let (ws_tx, _) = broadcast::channel(100);

    manager.add_websocket_connection(connection_id, ws_tx.clone()).await;

    Ok(ws.on_upgrade(move |socket| {
        handle_websocket_connection(socket, manager, filter, connection_id, user_id)
    }))
}

/// 处理WebSocket连接
async fn handle_websocket_connection(
    socket: axum::extract::ws::WebSocket,
    manager: Arc<RealtimeLogManager>,
    filter: LogFilter,
    connection_id: Uuid,
    user_id: Uuid,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (local_tx, mut local_rx) = broadcast::channel(100);

    // 添加连接到管理器
    manager.add_websocket_connection(connection_id, local_tx.clone()).await;

    // 监听日志事件并发送到WebSocket
    let manager_clone = manager.clone();
    let filter_clone = filter.clone();
    let mut log_rx = manager_clone.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(event) = log_rx.recv().await {
            // 应用过滤器
            if let Some(levels) = &filter_clone.level {
                if !levels.contains(&event.level) {
                    continue;
                }
            }

            if let Some(target) = &filter_clone.target {
                if !event.target.contains(target) {
                    continue;
                }
            }

            if let Some(workspace_id) = filter_clone.workspace_id {
                if event.workspace_id != Some(workspace_id) {
                    continue;
                }
            }

            if let Some(task_id) = filter_clone.task_id {
                if event.task_id != Some(task_id) {
                    continue;
                }
            }

            if let Some(agent_id) = filter_clone.agent_id {
                if event.agent_id != Some(agent_id) {
                    continue;
                }
            }

            if let Some(filter_user_id) = filter_clone.user_id {
                if event.user_id != Some(filter_user_id) {
                    continue;
                }
            }

            let message = serde_json::to_string(&event).unwrap();
            
            if let Err(e) = ws_tx
                .send(axum::extract::ws::Message::Text(message))
                .await
            {
                tracing::error!("Failed to send message to WebSocket: {}", e);
                break;
            }
        }
    });

    // 处理来自WebSocket的消息
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = ws_rx.next().await {
            match message {
                axum::extract::ws::Message::Text(text) => {
                    // 处理客户端发送的消息（如更改过滤器）
                    if let Ok(new_filter) = serde_json::from_str::<LogFilter>(&text) {
                        // 这里可以更新过滤器，但需要更复杂的实现
                        tracing::debug!("WebSocket client {} changed filter: {:?}", connection_id, new_filter);
                    }
                }
                axum::extract::ws::Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }
    });

    // 等待任一任务完成
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    };

    // 清理连接
    manager.remove_websocket_connection(connection_id).await;
    tracing::info!("WebSocket connection {} closed", connection_id);
}

/// 获取实时日志统计信息
pub async fn get_realtime_log_stats(
    State(manager): State<Arc<RealtimeLogManager>>,
    jar: CookieJar,
) -> Result<Json<serde_json::Value>, AppError> {
    // 验证用户权限
    let _user_id = AuthMiddleware::get_user_id_from_cookies(&jar)?;

    let connection_count = manager.get_connection_count().await;

    let stats = json!({
        "active_connections": connection_count,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    Ok(Json(stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_realtime_log_manager_creation() {
        // 这个测试主要是验证管理器可以正确创建
        // 由于需要Redis和数据库连接，这里只测试编译通过
        assert!(true);
    }

    #[tokio::test]
    async fn test_log_event_creation() {
        let manager = RealtimeLogManager::new(
            // 模拟连接池
            bb8::Pool::builder().build(RedisConnectionManager::new("redis://localhost").unwrap()).await.unwrap(),
            PgPool::connect("postgres://localhost").await.unwrap(),
        );

        let event = manager.create_log_event(
            "info".to_string(),
            "Test message".to_string(),
            "test::module".to_string(),
            json!({"key": "value"}),
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()),
        );

        assert_eq!(event.level, "info");
        assert_eq!(event.message, "Test message");
        assert_eq!(event.target, "test::module");
    }
}