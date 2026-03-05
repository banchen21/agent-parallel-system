use tokio::sync::broadcast;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use bb8_redis::RedisConnectionManager;
use sqlx::PgPool;

/// 实时日志事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeLogEvent {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    pub target: String,
}

/// 实时日志管理器
pub struct RealtimeLogManager {
    tx: broadcast::Sender<RealtimeLogEvent>,
}

impl RealtimeLogManager {
    pub fn new(_redis_pool: bb8::Pool<RedisConnectionManager>, _db_pool: PgPool) -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeLogEvent> {
        self.tx.subscribe()
    }

    pub async fn publish(&self, event: RealtimeLogEvent) -> Result<(), String> {
        self.tx.send(event).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_connection_count(&self) -> usize {
        self.tx.receiver_count()
    }
}
