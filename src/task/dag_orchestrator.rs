use actix::prelude::*;
use sqlx::PgPool;

use crate::{
    agsnets::actor_agents::AgentManagerActor,
    channel::actor_messages::ChannelManagerActor,
    task::model::{TaskItem, TaskTableModel},
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
    workspace_manager_actor: Addr<WorkspaceManageActor>
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
    type Result = ();

    fn handle(&mut self, msg: SubmitTask, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        let task = msg.task.clone();
        let task_id = uuid::Uuid::new_v4();
        let save_id = task_id.clone();

        let this_clone = self.clone();
        // 检测是否存在空工作空间
        tokio::spawn({
            let workspace_manager_actor = this_clone.workspace_manager_actor.clone();
            let user_name = msg.user_name.clone();
            async move {
                if let Err(e) = this.save_task_to_db(save_id, task.clone()).await {
                    tracing::error!("Failed to save task to database: {}", e);
                }
                let workspaces = workspace_manager_actor
                    .send(GetWorkspaces(user_name.clone()))
                    .await;
                match workspaces {
                    Ok(Ok(workspaces)) => {
                        if workspaces.is_empty() {
                            tracing::warn!("No workspaces found when submitting task");
                        } else {
                            // 兼容旧/新数据库：不依赖 `status` 字段，优先选择返回列表的第一个工作区
                            let workspace = workspaces.into_iter().next();
                            if workspace.is_none() {
                                tracing::warn!("No active workspace found for user: {}", user_name);
                                return;
                            }
                            let workspace = workspace.unwrap();
                            // 检测是否有空闲的agent
                            let agent_id = match this_clone
                                .agent_manager_actor
                                .send(crate::agsnets::actor_agents::CheckAvailableAgent {
                                    workspace_name: workspace.name.clone(),
                                    user_name: user_name.clone(),
                                })
                                .await
                            {
                                Ok(d) => match d {
                                    Ok(agent_id) => agent_id,
                                    Err(e) => {
                                        tracing::error!("Failed to check available agent: {}", e);
                                        return;
                                    }
                                },
                                Err(_) => {
                                    tracing::error!("Failed to send CheckAvailableAgent message");
                                    return;
                                }
                            };
                            // 将任务分配到工作空间与 Agent：更新任务记录的 workspace_name、assigned_agent_id、status
                            let pool = this_clone.pool.clone();
                            let workspace_name = workspace.name.clone();
                            let task_id = task_id.clone();
                            tokio::spawn(async move {
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
                            });
                        }
                    }
                    Ok(Err(e)) => tracing::error!("Failed to get workspaces: {}", e),
                    Err(e) => tracing::error!("Failed to send GetWorkspaces message: {}", e),
                }
            }
        });

        println!("Received task: {:?}", msg.task);
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
#[rtype(result = "Result<Vec<TaskTableModel>, anyhow::Error>")]
pub struct QueryAllTasks(pub String);

impl Handler<QueryAllTasks> for DagOrchestrator {
    type Result = ResponseFuture<Result<Vec<TaskTableModel>, anyhow::Error>>;

    fn handle(&mut self, msg: QueryAllTasks, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT id, depends_on, priority, status, name, description, workspace_name, assigned_agent_id, created_at FROM tasks WHERE workspace_name IN (SELECT name FROM workspaces WHERE owner_username = $1)",
            )
            .bind(msg.0)
            .fetch_all(&pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

            let tasks: Vec<TaskTableModel> =
                rows.into_iter().map(TaskTableModel::from_row).collect();
            Ok(tasks)
        })
    }
}
