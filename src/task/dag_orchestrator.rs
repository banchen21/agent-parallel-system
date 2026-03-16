use actix::prelude::*;
use sqlx::PgPool;

use crate::{
    agsnets::actor_agents_manage::AgentManagerActor,
    channel::actor_messages::ChannelManagerActor,
    task::model::{TaskInfo, TaskInfoResponse, TaskItem, TaskTableModel},
    workspace::workspace_actor::{GetWorkspaces, WorkspaceManageActor},
};

#[derive(Clone)]
pub struct DagOrchestrator {
    pool: PgPool,
    // Agent 管理器 Actor 地址
    agent_manager_actor: Addr<AgentManagerActor>,
    /// 消息通道
    channel_manager_actor: Addr<ChannelManagerActor>,
    // 工作空间
    workspace_manager_actor: Addr<WorkspaceManageActor>,
}

impl DagOrchestrator {
    pub fn new(
        pool: PgPool,
        agent_manager_actor: Addr<AgentManagerActor>,
        channel_manager_actor: Addr<ChannelManagerActor>,
        workspace_manager_actor: Addr<WorkspaceManageActor>,
    ) -> Self {
        Self {
            pool,
            agent_manager_actor,
            channel_manager_actor,
            workspace_manager_actor,
        }
    }

    // 1. 将任务保存到数据库，返回插入的 task_id
    pub async fn save_task_to_db(
        &self,
        task_id: uuid::Uuid,
        task: TaskItem,
    ) -> anyhow::Result<uuid::Uuid> {
        let pool = self.pool.clone();
        let task_row = TaskTableModel::from_task_item(task_id, task);
        tokio::spawn(async move {
            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO tasks (
                    id, depends_on, priority, status, name, description, workspace_name, assigned_agent_id, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                "#,
            )
            .bind(task_row.id)
            .bind(&task_row.depends_on)
            .bind(&task_row.priority)
            .bind(task_row.status.as_str())
            .bind(&task_row.name)
            .bind(&task_row.description)
            .bind(&task_row.workspace_name)
            .bind(&task_row.assigned_agent_id)
            .bind(task_row.created_at)
            .execute(&pool)
            .await
            {
                tracing::error!("Failed to save task to database: {}", e);
            } else {
                tracing::debug!("Task saved to database successfully: {:?}", task_row);
            }
        });
        Ok(task_id)
    }
}

// Actor 定义
impl Actor for DagOrchestrator {
    type Context = Context<Self>;
}

// 提交任务
#[derive(Message)]
#[rtype(result = "()")]
pub struct SubmitTask {
    pub user_name: String,
    pub task: TaskItem,
}

impl Handler<SubmitTask> for DagOrchestrator {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: SubmitTask, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let agent_manager = self.agent_manager_actor.clone();
        let workspace_manager = self.workspace_manager_actor.clone();
        let task = msg.task.clone();
        let user_name = msg.user_name.clone();

        Box::pin(async move {
            // 1) 插入任务（同步）
            let task_id = uuid::Uuid::new_v4();
            let insert_res = sqlx::query(
                r#"INSERT INTO tasks (id, depends_on, priority, status, name, description, workspace_name, assigned_agent_id, created_at)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            )
            .bind(task_id)
            .bind(&Vec::<uuid::Uuid>::new())
            .bind(&task.priority.as_str())
            .bind("published")
            .bind(&task.name)
            .bind(&task.description)
            .bind::<Option<String>>(None)
            .bind::<Option<uuid::Uuid>>(None)
            .bind(chrono::Utc::now())
            .execute(&pool)
            .await;

            if let Err(e) = insert_res {
                tracing::error!("Failed to insert task to DB: {}", e);
                return;
            }

            // 2) 选择工作区
            let workspaces_res = workspace_manager.send(GetWorkspaces(user_name.clone())).await;
            let workspace_opt = match workspaces_res {
                Ok(Ok(ws)) => ws.into_iter().next(),
                Ok(Err(e)) => {
                    tracing::error!("GetWorkspaces returned error: {}", e);
                    None
                }
                Err(e) => {
                    tracing::error!("GetWorkspaces mailbox error: {}", e);
                    None
                }
            };

            let workspace_name = if let Some(ws) = workspace_opt {
                ws.name
            } else {
                tracing::warn!("No workspace found for user {} when submitting task", user_name);
                return;
            };

            // 3) 同步询问可用 agent
            let agent_res = agent_manager
                .send(crate::agsnets::actor_agents_manage::CheckAvailableAgent {
                    workspace_name: workspace_name.clone(),
                    user_name: user_name.clone(),
                })
                .await;

            let assigned_agent = match agent_res {
                Ok(Ok(agent_id)) => Some(agent_id),
                Ok(Err(e)) => {
                    tracing::error!("CheckAvailableAgent returned error: {}", e);
                    None
                }
                Err(e) => {
                    tracing::error!("CheckAvailableAgent mailbox error: {}", e);
                    None
                }
            };

            // 4) 更新任务表（同步）
            if let Some(agent_id) = assigned_agent {
                // 确保对应的 AgentActor 已经在内存中启动，这样它才能轮询并把任务状态从 accepted 切换为 executing
                let _ = agent_manager
                    .send(crate::agsnets::actor_agents_manage::StartAgent { agent_id })
                    .await;

                if let Err(e) = sqlx::query(
                    "UPDATE tasks SET workspace_name=$1, assigned_agent_id=$2, status=$3 WHERE id=$4",
                )
                .bind(workspace_name.clone())
                .bind(agent_id)
                .bind("accepted")
                .bind(task_id)
                .execute(&pool)
                .await
                {
                    tracing::error!("Failed to update task assignment: {}", e);
                } else {
                    tracing::info!("Task assigned {} -> {}", task_id, workspace_name);
                }
            } else {
                if let Err(e) = sqlx::query("UPDATE tasks SET workspace_name=$1 WHERE id=$2")
                    .bind(workspace_name.clone())
                    .bind(task_id)
                    .execute(&pool)
                    .await
                {
                    tracing::error!("Failed to update task workspace: {}", e);
                }
            }

        }
        .into_actor(self))
    }
}

// 保存任务到数据库
#[derive(Message)]
#[rtype(result = "Result<uuid::Uuid, anyhow::Error>")]
pub struct SaveTaskToDb {
    pub task_id: uuid::Uuid,
    pub task: TaskItem,
}

impl Handler<SaveTaskToDb> for DagOrchestrator {
    type Result = ResponseFuture<Result<uuid::Uuid, anyhow::Error>>;

    fn handle(&mut self, msg: SaveTaskToDb, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        Box::pin(async move { this.save_task_to_db(msg.task_id, msg.task).await })
    }
}

// 查询所有任务
#[derive(Message)]
#[rtype(result = "Result<Vec<TaskInfoResponse>, anyhow::Error>")]
pub struct QueryAllTasks(pub String);

impl Handler<QueryAllTasks> for DagOrchestrator {
    type Result = ResponseFuture<Result<Vec<TaskInfoResponse>, anyhow::Error>>;

    fn handle(&mut self, msg: QueryAllTasks, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            // LEFT JOIN agents 获取 assigned agent 的 name
            let rows = sqlx::query(
                "SELECT t.id, t.depends_on, t.priority, t.status, t.name, t.description, t.workspace_name, a.name AS assigned_agent_name, t.created_at FROM tasks t LEFT JOIN agents a ON t.assigned_agent_id = a.id WHERE t.workspace_name IN (SELECT name FROM workspaces WHERE owner_username = $1)",
            )
            .bind(msg.0)
            .fetch_all(&pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

            let tasks: Vec<TaskInfoResponse> =
                rows.into_iter().map(TaskInfoResponse::from_row).collect();
            Ok(tasks)
        })
    }
}

// 根据任务id查询任务
#[derive(Message)]
#[rtype(result = "Result<TaskInfoResponse, anyhow::Error>")]
pub struct QueryTaskById(pub uuid::Uuid);

impl Handler<QueryTaskById> for DagOrchestrator {
    type Result = ResponseFuture<Result<TaskInfoResponse, anyhow::Error>>;

    fn handle(&mut self, msg: QueryTaskById, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let row_opt = sqlx::query(
                "SELECT t.id, t.depends_on, t.priority, t.status, t.name, t.description, t.workspace_name, a.name AS assigned_agent_name, t.created_at FROM tasks t LEFT JOIN agents a ON t.assigned_agent_id = a.id WHERE t.id = $1",
            )
            .bind(msg.0)
            .fetch_optional(&pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

            if let Some(row) = row_opt {
                Ok(TaskInfoResponse::from_row(row))
            } else {
                Err(anyhow::anyhow!("task not found"))
            }
        })
    }
}
