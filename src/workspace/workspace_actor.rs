use actix::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use std::fs::remove_dir_all;
use tracing::info;

use crate::utils::workspace_path::{ensure_dir, workspace_agents_dir, workspace_dir};
use crate::workspace::model::WorkspaceError;

// ==================== Manage Actor 定义 ====================

pub struct WorkspaceActor {
    pool: sqlx::PgPool,
    workspace_id: String,
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
            sqlx::query_as::<_, WorkspaceInfo>(
                "SELECT w.id, w.name, w.description, w.owner_username, w.status, w.created_at,
                    (SELECT COUNT(*) FROM agents a WHERE a.workspace_name = w.name) AS agent_count,
                    (SELECT COUNT(*) FROM tasks t WHERE t.workspace_name = w.name) AS task_count
                FROM workspaces w",
            )
            .fetch_all(&pool)
            .await
        };

        ctx.wait(
            actix::fut::wrap_future(fut).map(|result, actor: &mut Self, _ctx| {
                if let Ok(workspaces) = result {
                    for ws in workspaces {
                        let ws_dir = workspace_dir(&ws.name);
                        let agents_dir = workspace_agents_dir(&ws.name);
                        let _ = ensure_dir(&ws_dir).and_then(|_| ensure_dir(&agents_dir));
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
    pub description: Option<String>,
    pub owner_username: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    // 工作区中的智能体数量
    #[sqlx(default)]
    pub agent_count: i64,
    // 工作区中的任务数量（如果有任务表的话）
    #[sqlx(default)]
    pub task_count: i64,
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
impl Handler<CreateWorkspace> for WorkspaceManageActor {
    type Result = ResponseActFuture<Self, Result<WorkspaceInfo, WorkspaceError>>;

    fn handle(&mut self, msg: CreateWorkspace, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let name_clone = msg.name.clone();

        // 仅限英文以及下划线
        if !msg.name.chars().all(|c| c.is_alphabetic() || c == '_') {
            return Box::pin(futures::future::err(WorkspaceError::Message(
                "工作区名称只能包含字母和下划线".to_string(),
            )));
        }

        // 构造异步数据库操作
        let fut = async move {
            sqlx::query_as::<_, WorkspaceInfo>(
                r#"
                INSERT INTO workspaces (name, description, owner_username, status)
                VALUES ($1, $2, $3, 'active')
                RETURNING id, name, description, owner_username, status, created_at
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
                    // 在工作区根目录下创建 .workspaces/<name>/ 与 .workspaces/<name>/agents/
                    let ws_dir = workspace_dir(&name_clone);
                    let agents_dir = workspace_agents_dir(&name_clone);
                    if let Err(e) = ensure_dir(&ws_dir).and_then(|_| ensure_dir(&agents_dir)) {
                        tracing::warn!(
                            workspace = %name_clone,
                            error = %e,
                            "创建工作区目录失败（数据库已创建）"
                        );
                    }
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
#[rtype(result = "Result<(), WorkspaceError>")]
pub struct DeleteWorkspace {
    pub name: String,
}
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
                    // 删除工作区目录
                    let ws_dir = workspace_dir(&name_clone);
                    let agents_dir = workspace_agents_dir(&name_clone);
                    if let Err(e) =
                        remove_dir_all(&ws_dir).and_then(|_| remove_dir_all(&agents_dir))
                    {
                        tracing::warn!(
                            workspace = %name_clone,
                            error = %e,
                            "删除工作区目录失败（数据库已删除）"
                        );
                    }
                    actor.workspaces.remove(&name_clone.clone());
                }
                res
            }),
        )
    }
}

/// 查询用户拥有的工作空间
#[derive(Message)]
#[rtype(result = "Result<Vec<WorkspaceInfo>, WorkspaceError>")]
pub struct GetWorkspaces(pub String);
/// 3. 处理查询工作区列表
impl Handler<GetWorkspaces> for WorkspaceManageActor {
    type Result = ResponseActFuture<Self, Result<Vec<WorkspaceInfo>, WorkspaceError>>;

    fn handle(&mut self, msg: GetWorkspaces, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let user_name = msg.0.clone();
        let fut = async move {
            sqlx::query_as::<_, WorkspaceInfo>(
                r#"
                SELECT 
                    w.id, w.name, w.description, w.owner_username, w.status, w.created_at,
                    (SELECT COUNT(*) FROM agents a WHERE a.workspace_name = w.name) AS agent_count,
                    (SELECT COUNT(*) FROM tasks t WHERE t.workspace_name = w.name) AS task_count
                FROM workspaces w
                WHERE w.owner_username = $1
                "#,
            )
            .bind(user_name) // 3. 绑定具体的用户名参数
            .fetch_all(&pool)
            .await
            .map_err(WorkspaceError::DatabaseError)
        };

        Box::pin(actix::fut::wrap_future::<_, Self>(fut))
    }
}
