use actix::{Actor, Context, Handler, Message, ResponseFuture};
use anyhow::Result;
use log::info;
use tracing::error;

use crate::api::user::model::{User, UserPublic};

pub struct UserManagerActor {
    pool: sqlx::PgPool,
}

impl Actor for UserManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("UserManager Actor 已启动");
    }
}

impl UserManagerActor {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

/// Actor 消息：创建新用户（注册）
#[derive(Message, Debug)]
#[rtype(result = "Result<User>")]
pub struct CreateUser {
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
}

// 处理器：创建用户
impl Handler<CreateUser> for UserManagerActor {
    type Result = ResponseFuture<Result<User>>;

    fn handle(&mut self, msg: CreateUser, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let user = sqlx::query_as::<_, User>(
                r#"
                INSERT INTO users (username, password_hash, email)
                VALUES ($1, $2, $3)
                RETURNING id, username, password_hash, email, created_at, updated_at
                "#,
            )
            .bind(msg.username)
            .bind(msg.password_hash)
            .bind(msg.email)
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                error!("❌ 用户创建失败: {}", e);
                anyhow::anyhow!("用户已存在或数据库错误")
            })?;

            Ok(user)
        })
    }
}

/// Actor 消息：根据用户名查找用户（登录/鉴权用）
#[derive(Message)]
#[rtype(result = "Result<Option<User>>")]
pub struct GetUserByUsername {
    pub username: String,
}

// 处理器：根据用户名查询（用于 Auth 中间件验证）
impl Handler<GetUserByUsername> for UserManagerActor {
    type Result = ResponseFuture<Result<Option<User>>>;

    fn handle(&mut self, msg: GetUserByUsername, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1 LIMIT 1")
                .bind(msg.username)
                .fetch_optional(&pool)
                .await
                .map_err(|e| {
                    error!("❌ 查询用户失败: {}", e);
                    anyhow::anyhow!("数据库查询失败")
                })?;

            Ok(user)
        })
    }
}

/// 更新用户的 Refresh Token
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UpdateRefreshToken {
    pub username: String,
    pub token: Option<String>,
}

impl Handler<UpdateRefreshToken> for UserManagerActor {
    type Result = ResponseFuture<Result<()>>;
    fn handle(&mut self, msg: UpdateRefreshToken, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            sqlx::query("UPDATE users SET refresh_token = $1 WHERE username = $2")
                .bind(msg.token)
                .bind(msg.username)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }
}

/// 根据 Refresh Token 查找用户
#[derive(Message)]
#[rtype(result = "Result<Option<User>>")]
pub struct GetUserByRefreshToken {
    pub refresh_token: String,
}

impl Handler<GetUserByRefreshToken> for UserManagerActor {
    type Result = ResponseFuture<Result<Option<User>>>;
    fn handle(&mut self, msg: GetUserByRefreshToken, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            let user =
                sqlx::query_as::<_, User>("SELECT * FROM users WHERE refresh_token = $1 LIMIT 1")
                    .bind(msg.refresh_token)
                    .fetch_optional(&pool)
                    .await?;
            Ok(user)
        })
    }
}

/// 查询用户列表（用于管理页面）
#[derive(Message)]
#[rtype(result = "Result<Vec<UserPublic>>")]
pub struct ListUsers;

impl Handler<ListUsers> for UserManagerActor {
    type Result = ResponseFuture<Result<Vec<UserPublic>>>;

    fn handle(&mut self, _msg: ListUsers, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let users = sqlx::query_as::<_, UserPublic>(
                r#"
                SELECT id, username, email, created_at, updated_at
                FROM users
                ORDER BY id DESC
                "#,
            )
            .fetch_all(&pool)
            .await
            .map_err(|e| {
                error!("❌ 查询用户列表失败: {}", e);
                anyhow::anyhow!("数据库查询失败")
            })?;

            Ok(users)
        })
    }
}

/// 删除用户及其关联数据（用于内置控制台）
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct DeleteUser {
    pub username: String,
}

impl Handler<DeleteUser> for UserManagerActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: DeleteUser, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let mut tx = pool.begin().await?;

            sqlx::query(
                r#"
                DELETE FROM tasks
                WHERE workspace_name IN (
                    SELECT name FROM workspaces WHERE owner_username = $1
                )
                "#,
            )
            .bind(&msg.username)
            .execute(&mut *tx)
            .await?;

            sqlx::query("DELETE FROM agents WHERE owner_username = $1")
                .bind(&msg.username)
                .execute(&mut *tx)
                .await?;

            sqlx::query("DELETE FROM workspaces WHERE owner_username = $1")
                .bind(&msg.username)
                .execute(&mut *tx)
                .await?;

            sqlx::query("DELETE FROM channel_messages WHERE username = $1")
                .bind(&msg.username)
                .execute(&mut *tx)
                .await?;

            let result = sqlx::query("DELETE FROM users WHERE username = $1")
                .bind(&msg.username)
                .execute(&mut *tx)
                .await?;

            if result.rows_affected() == 0 {
                return Err(anyhow::anyhow!("用户不存在"));
            }

            tx.commit().await?;
            Ok(())
        })
    }
}
