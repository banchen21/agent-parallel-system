use actix::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use thiserror::Error;
use tracing::info;

use crate::workspace::model::WorkspaceError;

// ==================== Manage Actor 定义 ====================

pub struct WorkspaceActor {
    pool: sqlx::PgPool,
    workspace_id: String, // 标识当前是哪个工作区
}

impl WorkspaceActor {
    pub fn new(pool: sqlx::PgPool, workspace_id: String) -> Self {
        Self { pool, workspace_id }
    }
}

impl Actor for WorkspaceActor {
    type Context = Context<Self>;
}

/// 工作区 Actor：管理所有相关的子 Actor
pub struct WorkspaceManageActor {
    pool: sqlx::PgPool,
    // 【修复 1】管理 Actor 保存的应该是子 Actor 的地址 Addr，而不是信息数据本身
    workspaces: HashMap<String, Addr<WorkspaceActor>>,
}

impl WorkspaceManageActor {
    /// 创建一个新的工作区 Actor 管理实例
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            workspaces: HashMap::new(),
        }
    }
}

impl Actor for WorkspaceManageActor {
    type Context = Context<Self>;

    /// 在 Actor 启动时，自动从数据库加载已有的工作区并启动对应的子 Actor
    fn started(&mut self, ctx: &mut Self::Context) {
        let pool = self.pool.clone();

        let fut = async move {
            sqlx::query_as::<_, WorkspaceInfo>("SELECT * FROM workspaces")
                .fetch_all(&pool)
                .await
        };

        ctx.wait(
            actix::fut::wrap_future(fut).map(|result, actor: &mut Self, _ctx| {
                if let Ok(workspaces) = result {
                    for ws in workspaces {
                        let addr = WorkspaceActor::new(actor.pool.clone(), ws.name.clone()).start();
                        actor.workspaces.insert(ws.name, addr);
                    }
                    info!(
                        "成功从数据库加载并启动了 {} 个工作区 Actor",
                        actor.workspaces.len()
                    );
                }
            }),
        );
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct WorkspaceInfo {
    pub id: i32,
    pub name: String,
    // 将 description 改为 Option 匹配 CreateWorkspace 里的参数并防止数据库可能的 NULL 报错
    pub description: Option<String>,
    pub owner_username: String,
    pub created_at: DateTime<Utc>,
}

// ==================== Handlers 实现 ====================

/// 创建工作区消息
#[derive(Message, Deserialize, Serialize)]
#[rtype(result = "Result<WorkspaceInfo, WorkspaceError>")]
pub struct CreateWorkspace {
    pub name: String,
    pub description: Option<String>,
    pub owner_username: String,
}

/// 1. 处理创建工作区
impl Handler<CreateWorkspace> for WorkspaceManageActor {
    type Result = ResponseActFuture<Self, Result<WorkspaceInfo, WorkspaceError>>;

    fn handle(&mut self, msg: CreateWorkspace, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let name_clone = msg.name.clone();

        // 仅限英文
        if !msg.name.chars().all(char::is_alphabetic) {
            return Box::pin(futures::future::err(WorkspaceError::Message(
                "工作区名称只能包含字母".to_string(),
            )));
        }

        // 构造异步数据库操作
        let fut = async move {
            sqlx::query_as::<_, WorkspaceInfo>(
                r#"
                INSERT INTO workspaces (name, description, owner_username)
                VALUES ($1, $2, $3)
                RETURNING id, name, description, owner_username, created_at
                "#,
            )
            .bind(&msg.name)
            .bind(&msg.description)
            .bind(&msg.owner_username)
            .fetch_one(&pool)
            .await
        };

        Box::pin(
            actix::fut::wrap_future(fut).map(move |res, actor: &mut Self, _ctx| match res {
                Ok(workspace_info) => {
                    // 【修复 1 附属】创建时如果不存在则启动一个新的子 Actor 进行管理
                    if !actor.workspaces.contains_key(&name_clone) {
                        let addr =
                            WorkspaceActor::new(actor.pool.clone(), name_clone.clone()).start();
                        actor.workspaces.insert(name_clone, addr);
                    }
                    Ok(workspace_info)
                }
                Err(e) => Err(WorkspaceError::DatabaseError(e)),
            }),
        )
    }
}

/// 删除工作区消息 (通过 name 删除)
#[derive(Message)]
// 【修复 2】消息返回类型和 Handler 的 Result 保持一致
#[rtype(result = "Result<(), WorkspaceError>")]
pub struct DeleteWorkspace {
    pub name: String,
}
/// 2. 处理删除工作区
impl Handler<DeleteWorkspace> for WorkspaceManageActor {
    type Result = ResponseActFuture<Self, Result<(), WorkspaceError>>;

    fn handle(&mut self, msg: DeleteWorkspace, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let name_clone = msg.name.clone();

        let fut = async move {
            let result = sqlx::query("DELETE FROM workspaces WHERE name = $1")
                .bind(&msg.name)
                .execute(&pool)
                .await;

            match result {
                Ok(res) if res.rows_affected() > 0 => Ok(()),
                Ok(_) => Err(WorkspaceError::Message(format!(
                    "工作区 '{}' 不存在",
                    msg.name
                ))),
                Err(e) => Err(WorkspaceError::DatabaseError(e)),
            }
        };

        Box::pin(
            actix::fut::wrap_future(fut).map(move |res, actor: &mut Self, _ctx| {
                if res.is_ok() {
                    actor.workspaces.remove(&name_clone);
                }
                res
            }),
        )
    }
}

/// 查询所有工作区消息
#[derive(Message)]
#[rtype(result = "Result<Vec<WorkspaceInfo>, WorkspaceError>")]
pub struct GetWorkspaces;

/// 3. 处理查询工作区列表
impl Handler<GetWorkspaces> for WorkspaceManageActor {
    type Result = ResponseActFuture<Self, Result<Vec<WorkspaceInfo>, WorkspaceError>>;

    fn handle(&mut self, _msg: GetWorkspaces, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        let fut = async move {
            // 【修复 3】查询字段和结构体要求一致，移除原来并不在结构体内的 path 和 updated_at 字段
            sqlx::query_as::<_, WorkspaceInfo>(
                "SELECT id, name, description, owner_username, created_at FROM workspaces ORDER BY id ASC",
            )
            .fetch_all(&pool)
            .await
            .map_err(WorkspaceError::DatabaseError)
        };

        Box::pin(actix::fut::wrap_future::<_, Self>(fut))
    }
}
