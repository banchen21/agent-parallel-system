use actix::prelude::*;
use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

/// 数据库管理器
pub struct DatabaseManager {
    pool: PgPool,
}

impl DatabaseManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 初始化数据库表 (拆分为多条执行，更稳定)
    pub async fn initialize_database(&self) -> Result<()> {
        info!("正在初始化数据库表结构...");

        // 创建 messages 表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id SERIAL PRIMARY KEY,
                "user" TEXT NOT NULL,
                source_ip TEXT NOT NULL,
                device_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 创建 users 表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id SERIAL PRIMARY KEY,
                username VARCHAR(50) UNIQUE NOT NULL,
                password_hash VARCHAR(255) NOT NULL,
                email VARCHAR(255) UNIQUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 添加索引
        sqlx::query(
            r#"
    CREATE INDEX IF NOT EXISTS idx_messages_user_created_at ON messages ("user", created_at DESC);
    "#,
        )
        .execute(&self.pool)
        .await?;

        info!("数据库表结构初始化完成");
        Ok(())
    }
}

impl Actor for DatabaseManager {
    type Context = Context<Self>;
}

// --- 消息定义 ---

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct InitializeDatabase;

// --- Handler 实现 ---

impl Handler<InitializeDatabase> for DatabaseManager {
    // 关键改动：使用 ResponseFuture 异步处理，不要用 block_on
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, _msg: InitializeDatabase, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let this = DatabaseManager::new(pool); // 或者直接调用内部逻辑

        Box::pin(async move { this.initialize_database().await })
    }
}
