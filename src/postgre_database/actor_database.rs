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
            CREATE TABLE IF NOT EXISTS channel_messages (
                id SERIAL PRIMARY KEY,
                username TEXT NOT NULL,
                source_ip TEXT NOT NULL,
                device_type TEXT NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'unread',
                content TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
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

        // 创建工作区表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workspaces (
                id SERIAL PRIMARY KEY,       
                name VARCHAR(255) UNIQUE NOT NULL,
                description TEXT,                                      
                owner_username  VARCHAR(50) NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'active',
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 添加索引： user + created_at
        sqlx::query(
            r#"
        CREATE INDEX IF NOT EXISTS idx_channel_messages_username_created_at ON channel_messages (username, created_at DESC);
        "#,
        )
        .execute(&self.pool)
        .await?;

        // 为 workspaces 表添加按所有者查询的索引
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workspaces_owner_username ON workspaces (owner_username);
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 创建 agents 表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agents (
                id UUID PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                kind VARCHAR(50) NOT NULL DEFAULT 'general',
                workspace_name VARCHAR(255) NOT NULL,
                owner_username  VARCHAR(50) NOT NULL,
                CONSTRAINT fk_agents_workspace FOREIGN KEY (workspace_name) REFERENCES workspaces(name) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 为 agents 表添加按工作区查询的索引
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_agents_workspace_name ON agents (workspace_name);
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 创建 tasks 表（任务状态持久化）
        // 任务表说明：
        // - `id`：任务唯一标识（UUID）
        // - `depends_on`：依赖任务的 UUID 列表，任务启动前需确保这些依赖任务已完成
        // - `priority`：任务优先级（如 low/medium/high），用于调度排序
        // - `status`：任务当前状态（published/accepted/executing/submitted/reviewing/completed_*）
        // - `name`/`description`/`task_key`：任务标题、描述与唯一键（便于 DAG 识别）
        // - `workspace_name`：所属工作区名称（外键关系通过应用层保证）
        // - `assigned_agent_id`：被分配的 Agent（UUID），可为空
        // - `created_at`：创建时间，便于监控和并发冲突检测
        // 注意：如果未来需要按 `workspace_name` 或 `task_key` 做高频查询，请为相应字段建立索引或唯一约束。
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id UUID PRIMARY KEY,
                depends_on UUID[] NOT NULL DEFAULT '{}',
                priority VARCHAR(20) NOT NULL DEFAULT 'medium',
                status VARCHAR(20) NOT NULL DEFAULT 'published',
                name VARCHAR(255) NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                workspace_name VARCHAR(255),
                assigned_agent_id UUID,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 兼容旧库：补齐 tasks 表新增字段与索引
        sqlx::query(
            r#"
            ALTER TABLE tasks
                ADD COLUMN IF NOT EXISTS depends_on UUID[] NOT NULL DEFAULT '{}',
                ADD COLUMN IF NOT EXISTS priority VARCHAR(20) NOT NULL DEFAULT 'medium',
                ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'published',
                ADD COLUMN IF NOT EXISTS name VARCHAR(255) NOT NULL DEFAULT '',
                ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '',
                ADD COLUMN IF NOT EXISTS workspace_name VARCHAR(255),
                ADD COLUMN IF NOT EXISTS assigned_agent_id UUID,
                ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_workspace_name_created_at
            ON tasks (workspace_name, created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_assigned_agent_status_created_at
            ON tasks (assigned_agent_id, status, created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 创建任务审阅结果表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_reviews (
                task_id UUID PRIMARY KEY,
                review_approved BOOLEAN NOT NULL DEFAULT FALSE,
                review_result TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                CONSTRAINT fk_task_reviews_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 兼容旧库：补齐 task_reviews 表字段
        sqlx::query(
            r#"
            ALTER TABLE task_reviews
                ADD COLUMN IF NOT EXISTS review_approved BOOLEAN NOT NULL DEFAULT FALSE,
                ADD COLUMN IF NOT EXISTS review_result TEXT NOT NULL DEFAULT '',
                ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_task_reviews_created_at
            ON task_reviews (created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 设置时区
        sqlx::query(
            r#"
    ALTER DATABASE agent_system SET timezone = 'Asia/Shanghai';
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
