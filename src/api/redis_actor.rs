use actix::prelude::*;
use anyhow::Result;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use tracing::{debug, error, info};

/// Redis 管理器 Actor
pub struct RedisActor {
    // 使用 ConnectionManager，它支持自动重连且是多路复用的
    conn_manager: ConnectionManager,
}

impl RedisActor {
    /// 异步初始化方法
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        // 调用你提供的源码中的 get_connection_manager()
        let conn_manager = client.get_connection_manager().await?;
        Ok(Self { conn_manager })
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("RedisActor 已启动 (使用 ConnectionManager)");
    }
}

// --- 消息定义 ---

/// 存储 Refresh Token
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SaveRefreshToken {
    pub token: String,
    pub username: String,
    pub expires_in_seconds: u64,
}

/// 验证并消耗（删除）Token (Token 旋转策略)
#[derive(Message)]
#[rtype(result = "Result<Option<String>>")]
pub struct VerifyAndConsumeToken {
    pub token: String,
}

/// 删除 Token (用于强制退出)
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct DeleteToken {
    pub token: String,
}

// --- Handler 实现 ---

impl Handler<SaveRefreshToken> for RedisActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: SaveRefreshToken, _ctx: &mut Self::Context) -> Self::Result {
        let mut conn = self.conn_manager.clone();
        Box::pin(async move {
            let key = format!("rt:{}", msg.token);
            // SETEX: 设置值和过期时间（秒）
            let _: () = conn
                .set_ex(&key, msg.username, msg.expires_in_seconds)
                .await?;
            Ok(())
        })
    }
}

impl Handler<VerifyAndConsumeToken> for RedisActor {
    type Result = ResponseFuture<Result<Option<String>>>;

    fn handle(&mut self, msg: VerifyAndConsumeToken, _ctx: &mut Self::Context) -> Self::Result {
        let mut conn = self.conn_manager.clone();
        Box::pin(async move {
            let key = format!("rt:{}", msg.token);

            // 1. 尝试获取关联的用户名
            let username: Option<String> = conn.get(&key).await?;

            // 2. 如果存在，立即删除（确保 Token 只能使用一次，即 Token Rotation）
            if username.is_some() {
                let _: () = conn.del(&key).await?;
                debug!("Redis: 已消耗 Token，Key: {}", key);
            }

            Ok(username)
        })
    }
}

impl Handler<DeleteToken> for RedisActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: DeleteToken, _ctx: &mut Self::Context) -> Self::Result {
        let mut conn = self.conn_manager.clone();
        Box::pin(async move {
            let key = format!("rt:{}", msg.token);
            let _: () = conn.del(&key).await?;
            Ok(())
        })
    }
}
