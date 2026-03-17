use actix::prelude::*;
use sqlx::PgPool;
use sqlx::Row;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    agsnets::actor_agents_manage::AgentManagerActor,
    channel::actor_messages::{ChannelManagerActor, SaveMessage},
    chat::model::{MessageContent, UserMessage},
    task::model::{TaskInfoResponse, TaskItem, TaskTableModel},
    task::task_agent::{ReviewSubmittedTask, TaskAgent},
    workspace::workspace_actor::{GetWorkspaces, WorkspaceManageActor},
};

#[derive(Clone)]
pub struct DagOrchestrActor {
    pool: PgPool,
    // Agent 管理器 Actor 地址
    agent_manager_actor: Addr<AgentManagerActor>,
    /// 消息通道
    channel_manager_actor: Addr<ChannelManagerActor>,
    // 工作空间
    workspace_manager_actor: Addr<WorkspaceManageActor>,
    /// 任务审阅者（TaskAgent）
    task_reviewer_actor: Option<Addr<TaskAgent>>,
    /// 首次投递失败的审阅消息缓存，后续由定时器自动重试
    pending_review_deliveries: HashMap<uuid::Uuid, PendingReviewDelivery>,
    submitted_recover_scan_interval_secs: u64,
    first_retry_delay_secs: u64,
}

#[derive(Clone)]
struct PendingReviewDelivery {
    msg: ReviewSubmittedTask,
    next_retry_at: Instant,
    retry_count: u32,
}

impl DagOrchestrActor {
    pub fn new(
        pool: PgPool,
        agent_manager_actor: Addr<AgentManagerActor>,
        channel_manager_actor: Addr<ChannelManagerActor>,
        workspace_manager_actor: Addr<WorkspaceManageActor>,
        submitted_recover_scan_interval_secs: u64,
        first_retry_delay_secs: u64,
    ) -> Self {
        Self {
            pool,
            agent_manager_actor,
            channel_manager_actor,
            workspace_manager_actor,
            task_reviewer_actor: None,
            pending_review_deliveries: HashMap::new(),
            submitted_recover_scan_interval_secs,
            first_retry_delay_secs,
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

    async fn on_task_completed_success(&self, task_id: uuid::Uuid) {
        tracing::info!(
            "Task {} completed_success: reward hook reserved for future blockchain integration",
            task_id
        );
    }

    async fn on_task_completed_failure(&self, task_id: uuid::Uuid) {
        tracing::info!(
            "Task {} completed_failure: penalty hook reserved for future blockchain integration",
            task_id
        );
    }

    fn build_task_markdown_message(task_title: &str, agent_name: &str, event: &str, detail: &str) -> String {
        format!(
            "### 任务系统通知\n- 任务标题: {}\n- 执行任务的智能体: {}\n- 事件: {}\n\n{}",
            task_title, agent_name, event, detail
        )
    }
}

// Actor 定义
impl Actor for DagOrchestrActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // 回补历史遗留 submitted 任务，避免旧版本或瞬时投递失败造成永久卡住。
        let recover_interval = self.submitted_recover_scan_interval_secs.max(5);
        ctx.run_interval(Duration::from_secs(recover_interval), |_actor, ctx| {
            ctx.notify(RecoverSubmittedReviews);
        });

        ctx.run_interval(Duration::from_secs(15), |actor, _ctx| {
            if actor.pending_review_deliveries.is_empty() {
                return;
            }

            let Some(task_reviewer) = actor.task_reviewer_actor.clone() else {
                tracing::warn!(
                    "Task reviewer not registered, pending review deliveries: {}",
                    actor.pending_review_deliveries.len()
                );
                return;
            };

            let now = Instant::now();
            let due_task_ids: Vec<uuid::Uuid> = actor
                .pending_review_deliveries
                .iter()
                .filter_map(|(task_id, pending)| {
                    if now >= pending.next_retry_at {
                        Some(*task_id)
                    } else {
                        None
                    }
                })
                .collect();

            for task_id in due_task_ids {
                let Some(pending) = actor.pending_review_deliveries.get_mut(&task_id) else {
                    continue;
                };

                match task_reviewer.try_send(pending.msg.clone()) {
                    Ok(_) => {
                        actor.pending_review_deliveries.remove(&task_id);
                        tracing::info!("Review delivery retry succeeded for task {}", task_id);
                    }
                    Err(e) => {
                        pending.retry_count = pending.retry_count.saturating_add(1);
                        let backoff_secs = (60u64)
                            .saturating_mul(2u64.saturating_pow(pending.retry_count.min(4)));
                        pending.next_retry_at = Instant::now() + Duration::from_secs(backoff_secs);
                        tracing::warn!(
                            "Review delivery retry failed for task {} (retry_count={}): {}",
                            task_id,
                            pending.retry_count,
                            e
                        );
                    }
                }
            }
        });
    }
}

// 提交任务
#[derive(Message)]
#[rtype(result = "()")]
pub struct SubmitTask {
    pub user_name: String,
    pub task: TaskItem,
}

impl Handler<SubmitTask> for DagOrchestrActor {
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

impl Handler<SaveTaskToDb> for DagOrchestrActor {
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

impl Handler<QueryAllTasks> for DagOrchestrActor {
    type Result = ResponseFuture<Result<Vec<TaskInfoResponse>, anyhow::Error>>;

    fn handle(&mut self, msg: QueryAllTasks, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            // LEFT JOIN agents 获取 assigned agent 的 name
            let rows = sqlx::query(
                "SELECT t.id, t.depends_on, t.priority, t.status, t.name, t.description, t.workspace_name, a.name AS assigned_agent_name, tr.review_result, tr.review_approved, t.created_at FROM tasks t LEFT JOIN agents a ON t.assigned_agent_id = a.id LEFT JOIN task_reviews tr ON tr.task_id = t.id WHERE t.workspace_name IN (SELECT name FROM workspaces WHERE owner_username = $1)",
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

impl Handler<QueryTaskById> for DagOrchestrActor {
    type Result = ResponseFuture<Result<TaskInfoResponse, anyhow::Error>>;

    fn handle(&mut self, msg: QueryTaskById, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let row_opt = sqlx::query(
                "SELECT t.id, t.depends_on, t.priority, t.status, t.name, t.description, t.workspace_name, a.name AS assigned_agent_name, tr.review_result, tr.review_approved, t.created_at FROM tasks t LEFT JOIN agents a ON t.assigned_agent_id = a.id LEFT JOIN task_reviews tr ON tr.task_id = t.id WHERE t.id = $1",
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

#[derive(Debug, Clone)]
pub struct PendingReviewContext {
    pub task_id: uuid::Uuid,
    pub task_name: String,
    pub task_description: String,
    pub review_approved: bool,
    pub review_result: String,
}

#[derive(Message)]
#[rtype(result = "Result<Option<PendingReviewContext>, anyhow::Error>")]
pub struct QueryLatestReviewingTaskByUser {
    pub user_name: String,
}

impl Handler<QueryLatestReviewingTaskByUser> for DagOrchestrActor {
    type Result = ResponseFuture<Result<Option<PendingReviewContext>, anyhow::Error>>;

    fn handle(&mut self, msg: QueryLatestReviewingTaskByUser, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            let row = sqlx::query(
                r#"
                SELECT t.id, t.name, t.description, t.assigned_agent_id, tr.review_approved, tr.review_result
                FROM tasks t
                JOIN workspaces w ON w.name = t.workspace_name
                LEFT JOIN task_reviews tr ON tr.task_id = t.id
                WHERE w.owner_username = $1 AND t.status = 'under_review'
                ORDER BY t.created_at DESC
                LIMIT 1
                "#,
            )
            .bind(&msg.user_name)
            .fetch_optional(&pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

            let Some(row) = row else {
                return Ok(None);
            };

            Ok(Some(PendingReviewContext {
                task_id: row.get("id"),
                task_name: row.get("name"),
                task_description: row.get("description"),
                review_approved: row.get::<Option<bool>, _>("review_approved").unwrap_or(false),
                review_result: row.get::<Option<String>, _>("review_result").unwrap_or_default(),
            }))
        })
    }
}

/// Agent 执行事件通知（用于写入通道消息，便于前端/订阅方观测任务推进）。
#[derive(Message)]
#[rtype(result = "()")] 
pub struct NotifyTaskExecutionEvent {
    pub task_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub phase: String,
    pub success: Option<bool>,
    pub executed: Option<bool>,
    pub should_retry: Option<bool>,
    pub selected_tool_id: Option<String>,
    pub interpreted_output: Option<String>,
    pub raw_output: Option<String>,
    pub failure_reason: Option<String>,
    pub retry_in_secs: Option<u64>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct RecoverSubmittedReviews;

#[derive(Message)]
#[rtype(result = "()")]
pub struct RegisterTaskReviewer {
    pub task_agent: Addr<TaskAgent>,
}

impl Handler<RegisterTaskReviewer> for DagOrchestrActor {
    type Result = ();

    fn handle(&mut self, msg: RegisterTaskReviewer, _ctx: &mut Self::Context) -> Self::Result {
        self.task_reviewer_actor = Some(msg.task_agent);
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), anyhow::Error>")]
pub struct BeginTaskReview {
    pub task_id: uuid::Uuid,
}

impl Handler<BeginTaskReview> for DagOrchestrActor {
    type Result = ResponseFuture<Result<(), anyhow::Error>>;

    fn handle(&mut self, msg: BeginTaskReview, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(async move {
            sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2 AND status = $3")
                .bind("under_review")
                .bind(msg.task_id)
                .bind("submitted")
                .execute(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), anyhow::Error>")]
pub struct CompleteTaskReview {
    pub task_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub approved: bool,
    pub review_result: String,
}

impl Handler<CompleteTaskReview> for DagOrchestrActor {
    type Result = ResponseActFuture<Self, Result<(), anyhow::Error>>;

    fn handle(&mut self, msg: CompleteTaskReview, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let channel = self.channel_manager_actor.clone();
        Box::pin(
            async move {
                sqlx::query(
                    r#"
                    INSERT INTO task_reviews (task_id, review_approved, review_result)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (task_id)
                    DO UPDATE SET review_approved = EXCLUDED.review_approved, review_result = EXCLUDED.review_result, created_at = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(msg.task_id)
                .bind(msg.approved)
                .bind(&msg.review_result)
                .execute(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

                let summary = if msg.approved {
                    format!("任务进入审阅决策阶段。Agent任务执行结果：{}\n\n系统判定：建议通过。", msg.review_result)
                } else {
                    format!("任务进入审阅决策阶段。Agent任务执行结果：{}\n\n系统判定：建议不通过。", msg.review_result)
                };

                let meta_row = sqlx::query(
                    r#"
                    SELECT t.name AS task_name, a.name AS agent_name
                    FROM tasks t
                    LEFT JOIN agents a ON a.id = $2
                    WHERE t.id = $1
                    "#,
                )
                .bind(msg.task_id)
                .bind(msg.agent_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

                let task_title = meta_row
                    .as_ref()
                    .and_then(|r| r.get::<Option<String>, _>("task_name"))
                    .unwrap_or_else(|| msg.task_id.to_string());
                let agent_name = meta_row
                    .as_ref()
                    .and_then(|r| r.get::<Option<String>, _>("agent_name"))
                    .unwrap_or_else(|| msg.agent_id.to_string());

                let text = DagOrchestrActor::build_task_markdown_message(
                    &task_title,
                    &agent_name,
                    "任务进入审阅决策阶段",
                    &summary,
                );
                let message = UserMessage {
                    sender: "任务系统".to_string(),
                    source_ip: "system".to_string(),
                    device_type: "agent".to_string(),
                    content: MessageContent::Text(text),
                    created_at: chrono::Utc::now(),
                };
                channel.do_send(SaveMessage { message });

                Ok(())
            }
            .into_actor(self),
        )
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), anyhow::Error>")]
pub struct FinalizeTaskDecision {
    pub task_id: uuid::Uuid,
    pub approved: bool,
    pub decision_reason: String,
}

#[derive(Message)]
#[rtype(result = "Result<TaskInfoResponse, anyhow::Error>")]
pub struct QueryTaskDetailById {
    pub task_id: uuid::Uuid,
    pub user_name: String,
}

impl Handler<QueryTaskDetailById> for DagOrchestrActor {
    type Result = ResponseFuture<Result<TaskInfoResponse, anyhow::Error>>;

    fn handle(&mut self, msg: QueryTaskDetailById, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let row_opt = sqlx::query(
                r#"
                SELECT
                    t.id,
                    t.depends_on,
                    t.priority,
                    t.status,
                    t.name,
                    t.description,
                    t.workspace_name,
                    a.name AS assigned_agent_name,
                    tr.review_result,
                    tr.review_approved,
                    t.created_at
                FROM tasks t
                JOIN workspaces w ON w.name = t.workspace_name
                LEFT JOIN agents a ON t.assigned_agent_id = a.id
                LEFT JOIN task_reviews tr ON tr.task_id = t.id
                WHERE t.id = $1 AND w.owner_username = $2
                "#,
            )
            .bind(msg.task_id)
            .bind(msg.user_name)
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

#[derive(Message)]
#[rtype(result = "Result<TaskInfoResponse, anyhow::Error>")]
pub struct ResolveTaskReviewDecision {
    pub task_id: uuid::Uuid,
    pub user_name: String,
    pub accept: bool,
}

#[derive(Message)]
#[rtype(result = "Result<(), anyhow::Error>")]
pub struct DeleteTaskById {
    pub task_id: uuid::Uuid,
    pub user_name: String,
}

impl Handler<ResolveTaskReviewDecision> for DagOrchestrActor {
    type Result = ResponseActFuture<Self, Result<TaskInfoResponse, anyhow::Error>>;

    fn handle(&mut self, msg: ResolveTaskReviewDecision, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let channel = self.channel_manager_actor.clone();
        let agent_manager = self.agent_manager_actor.clone();
        let this = self.clone();

        Box::pin(
            async move {
                let row = sqlx::query(
                    r#"
                    SELECT
                        t.id,
                        t.name,
                        t.assigned_agent_id,
                        t.workspace_name,
                        t.status,
                        a.name AS assigned_agent_name,
                        tr.review_approved,
                        tr.review_result
                    FROM tasks t
                    JOIN workspaces w ON w.name = t.workspace_name
                    LEFT JOIN agents a ON a.id = t.assigned_agent_id
                    LEFT JOIN task_reviews tr ON tr.task_id = t.id
                    WHERE t.id = $1 AND w.owner_username = $2
                    "#,
                )
                .bind(msg.task_id)
                .bind(&msg.user_name)
                .fetch_optional(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

                let Some(row) = row else {
                    return Err(anyhow::anyhow!("task not found"));
                };

                let current_status: String = row.get("status");
                if current_status != "under_review" {
                    return Err(anyhow::anyhow!("task is not under review"));
                }

                let agent_id: Option<uuid::Uuid> = row.get("assigned_agent_id");
                let task_title: String = row.get("name");
                let agent_name = row
                    .get::<Option<String>, _>("assigned_agent_name")
                    .unwrap_or_else(|| agent_id.map(|v| v.to_string()).unwrap_or_else(|| "未分配".to_string()));
                let workspace_name: String = row.get("workspace_name");
                let review_approved = row.get::<Option<bool>, _>("review_approved").unwrap_or(false);
                let review_result = row.get::<Option<String>, _>("review_result").unwrap_or_default();

                if msg.accept {
                    let final_status = if review_approved {
                        "completed_success"
                    } else {
                        "completed_failure"
                    };

                    sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2 AND status = $3")
                        .bind(final_status)
                        .bind(msg.task_id)
                        .bind("under_review")
                        .execute(&pool)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;

                    if review_approved {
                        this.on_task_completed_success(msg.task_id).await;
                    } else {
                        this.on_task_completed_failure(msg.task_id).await;
                    }

                    let summary = if review_approved {
                        format!("用户已接收审阅结果，任务完成。审阅意见：{}", review_result)
                    } else {
                        format!("用户已接收审阅结果，任务判定失败。审阅意见：{}", review_result)
                    };
                    let text = DagOrchestrActor::build_task_markdown_message(
                        &task_title,
                        &agent_name,
                        "用户接收审阅结果",
                        &summary,
                    );
                    channel.do_send(SaveMessage {
                        message: UserMessage {
                            sender: "任务系统".to_string(),
                            source_ip: "system".to_string(),
                            device_type: "agent".to_string(),
                            content: MessageContent::Text(text),
                            created_at: chrono::Utc::now(),
                        },
                    });
                } else {
                    sqlx::query(
                        "UPDATE tasks SET status = $1, assigned_agent_id = NULL WHERE id = $2 AND status = $3",
                    )
                    .bind("published")
                    .bind(msg.task_id)
                    .bind("under_review")
                    .execute(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                    sqlx::query("DELETE FROM task_reviews WHERE task_id = $1")
                        .bind(msg.task_id)
                        .execute(&pool)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;

                    let text = DagOrchestrActor::build_task_markdown_message(
                        &task_title,
                        &agent_name,
                        "用户拒绝审阅结果",
                        "任务已重置为 published，并重新进入分配流程。",
                    );
                    channel.do_send(SaveMessage {
                        message: UserMessage {
                            sender: "任务系统".to_string(),
                            source_ip: "system".to_string(),
                            device_type: "agent".to_string(),
                            content: MessageContent::Text(text),
                            created_at: chrono::Utc::now(),
                        },
                    });

                    // 尝试将任务重新分配给空闲 Agent
                    let reassign_res = agent_manager
                        .send(crate::agsnets::actor_agents_manage::CheckAvailableAgent {
                            workspace_name: workspace_name.clone(),
                            user_name: msg.user_name.clone(),
                        })
                        .await;

                    if let Ok(Ok(new_agent_id)) = reassign_res {
                        let _ = agent_manager
                            .send(crate::agsnets::actor_agents_manage::StartAgent { agent_id: new_agent_id })
                            .await;

                        if let Err(e) = sqlx::query(
                            "UPDATE tasks SET assigned_agent_id=$1, status=$2 WHERE id=$3",
                        )
                        .bind(new_agent_id)
                        .bind("accepted")
                        .bind(msg.task_id)
                        .execute(&pool)
                        .await
                        {
                            tracing::error!("Failed to reassign task {} after rejection: {}", msg.task_id, e);
                        } else {
                            tracing::info!("Task {} reassigned to agent {} after rejection", msg.task_id, new_agent_id);
                        }
                    } else {
                        tracing::info!("No available agent for task {} after rejection, task stays published", msg.task_id);
                    }
                }

                let detail_row = sqlx::query(
                    r#"
                    SELECT
                        t.id,
                        t.depends_on,
                        t.priority,
                        t.status,
                        t.name,
                        t.description,
                        t.workspace_name,
                        a.name AS assigned_agent_name,
                        tr.review_result,
                        tr.review_approved,
                        t.created_at
                    FROM tasks t
                    JOIN workspaces w ON w.name = t.workspace_name
                    LEFT JOIN agents a ON t.assigned_agent_id = a.id
                    LEFT JOIN task_reviews tr ON tr.task_id = t.id
                    WHERE t.id = $1 AND w.owner_username = $2
                    "#,
                )
                .bind(msg.task_id)
                .bind(msg.user_name)
                .fetch_one(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

                Ok(TaskInfoResponse::from_row(detail_row))
            }
            .into_actor(self),
        )
    }
}

impl Handler<DeleteTaskById> for DagOrchestrActor {
    type Result = ResponseFuture<Result<(), anyhow::Error>>;

    fn handle(&mut self, msg: DeleteTaskById, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let row_opt = sqlx::query(
                r#"
                SELECT t.status
                FROM tasks t
                JOIN workspaces w ON w.name = t.workspace_name
                WHERE t.id = $1 AND w.owner_username = $2
                "#,
            )
            .bind(msg.task_id)
            .bind(&msg.user_name)
            .fetch_optional(&pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

            let Some(row) = row_opt else {
                return Err(anyhow::anyhow!("task not found"));
            };

            let status: String = row.get("status");
            if status != "completed_success" && status != "completed_failure" && status != "cancelled" {
                return Err(anyhow::anyhow!("only completed tasks can be deleted"));
            }

            sqlx::query("DELETE FROM task_reviews WHERE task_id = $1")
                .bind(msg.task_id)
                .execute(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            sqlx::query("DELETE FROM tasks WHERE id = $1")
                .bind(msg.task_id)
                .execute(&pool)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            Ok(())
        })
    }
}

impl Handler<FinalizeTaskDecision> for DagOrchestrActor {
    type Result = ResponseActFuture<Self, Result<(), anyhow::Error>>;

    fn handle(&mut self, msg: FinalizeTaskDecision, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let channel = self.channel_manager_actor.clone();
        let this = self.clone();
        Box::pin(
            async move {
                let row = sqlx::query(
                    r#"
                    SELECT t.name, t.assigned_agent_id, a.name AS assigned_agent_name
                    FROM tasks t
                    LEFT JOIN agents a ON a.id = t.assigned_agent_id
                    WHERE t.id = $1
                    "#,
                )
                    .bind(msg.task_id)
                    .fetch_one(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                let agent_id: Option<uuid::Uuid> = row.get("assigned_agent_id");
                let task_title: String = row.get("name");
                let agent_name = row
                    .get::<Option<String>, _>("assigned_agent_name")
                    .unwrap_or_else(|| agent_id.map(|v| v.to_string()).unwrap_or_else(|| "未分配".to_string()));

                let final_status = if msg.approved {
                    "completed_success"
                } else {
                    "completed_failure"
                };

                sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2 AND status = $3")
                    .bind(final_status)
                    .bind(msg.task_id)
                    .bind("under_review")
                    .execute(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                if msg.approved {
                    this.on_task_completed_success(msg.task_id).await;
                } else {
                    this.on_task_completed_failure(msg.task_id).await;
                }

                let summary = if msg.approved {
                    format!("用户确认任务完成，任务已通过。决策依据：{}", msg.decision_reason)
                } else {
                    format!("用户判定任务未完成，任务已驳回。决策依据：{}", msg.decision_reason)
                };

                let text = DagOrchestrActor::build_task_markdown_message(
                    &task_title,
                    &agent_name,
                    "用户最终决策",
                    &summary,
                );
                let message = UserMessage {
                    sender: "任务系统".to_string(),
                    source_ip: "system".to_string(),
                    device_type: "agent".to_string(),
                    content: MessageContent::Text(text),
                    created_at: chrono::Utc::now(),
                };
                channel.do_send(SaveMessage { message });

                Ok(())
            }
            .into_actor(self),
        )
    }
}

impl Handler<RecoverSubmittedReviews> for DagOrchestrActor {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, _msg: RecoverSubmittedReviews, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(
            async move {
                sqlx::query(
                    r#"
                    SELECT t.id, t.assigned_agent_id
                    FROM tasks t
                    LEFT JOIN task_reviews tr ON tr.task_id = t.id
                    WHERE t.status = 'submitted' AND tr.task_id IS NULL
                    ORDER BY t.created_at ASC
                    LIMIT 50
                    "#,
                )
                .fetch_all(&pool)
                .await
            }
            .into_actor(self)
            .map(|res, actor, _ctx| {
                let rows = match res {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("RecoverSubmittedReviews query failed: {}", e);
                        return;
                    }
                };

                if rows.is_empty() {
                    return;
                }

                let Some(task_reviewer) = actor.task_reviewer_actor.clone() else {
                    tracing::warn!(
                        "RecoverSubmittedReviews found {} tasks but reviewer is not registered",
                        rows.len()
                    );
                    return;
                };

                for row in rows {
                    let task_id: uuid::Uuid = row.get("id");

                    if actor.pending_review_deliveries.contains_key(&task_id) {
                        continue;
                    }

                    let agent_id: Option<uuid::Uuid> = row.get("assigned_agent_id");
                    let review_msg = ReviewSubmittedTask {
                        task_id,
                        agent_id: agent_id.unwrap_or_else(uuid::Uuid::nil),
                        selected_tool_id: None,
                        interpreted_output: Some(
                            "系统恢复：该任务此前停留在 submitted，已自动重试进入审阅流程。"
                                .to_string(),
                        ),
                        raw_output: None,
                        failure_reason: None,
                        executed: true,
                        should_retry: false,
                    };

                    match task_reviewer.try_send(review_msg.clone()) {
                        Ok(_) => {
                            tracing::info!(
                                "RecoverSubmittedReviews resent review for task {}",
                                task_id
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "RecoverSubmittedReviews enqueue failed for task {}: {}",
                                task_id,
                                e
                            );
                            actor.pending_review_deliveries.insert(
                                task_id,
                                PendingReviewDelivery {
                                    msg: review_msg,
                                    next_retry_at: Instant::now() + Duration::from_secs(60),
                                    retry_count: 0,
                                },
                            );
                        }
                    }
                }
            }),
        )
    }
}

impl Handler<NotifyTaskExecutionEvent> for DagOrchestrActor {
    type Result = ();

    fn handle(&mut self, msg: NotifyTaskExecutionEvent, _ctx: &mut Self::Context) -> Self::Result {
        if msg.phase == "mcp_result" && msg.success == Some(true) {
            let review_msg = ReviewSubmittedTask {
                task_id: msg.task_id,
                agent_id: msg.agent_id,
                selected_tool_id: msg.selected_tool_id.clone(),
                interpreted_output: msg.interpreted_output.clone(),
                raw_output: msg.raw_output.clone(),
                failure_reason: msg.failure_reason.clone(),
                executed: msg.executed.unwrap_or(false),
                should_retry: msg.should_retry.unwrap_or(false),
            };

            if let Some(task_reviewer) = &self.task_reviewer_actor {
                match task_reviewer.try_send(review_msg.clone()) {
                    Ok(_) => {
                        self.pending_review_deliveries.remove(&msg.task_id);
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Initial review delivery failed for task {}, will retry: {}",
                            msg.task_id,
                            e
                        );
                        self.pending_review_deliveries.insert(
                            msg.task_id,
                            PendingReviewDelivery {
                                msg: review_msg,
                                next_retry_at: Instant::now()
                                    + Duration::from_secs(self.first_retry_delay_secs.max(5)),
                                retry_count: 0,
                            },
                        );
                    }
                }
            } else {
                tracing::warn!(
                    "Task reviewer not registered when handling task {}, queued for retry",
                    msg.task_id
                );
                self.pending_review_deliveries.insert(
                    msg.task_id,
                    PendingReviewDelivery {
                        msg: review_msg,
                        next_retry_at: Instant::now()
                            + Duration::from_secs(self.first_retry_delay_secs.max(5)),
                        retry_count: 0,
                    },
                );
            }
        }

        let tool_id = msg
            .selected_tool_id
            .clone()
            .unwrap_or_else(|| "unknown_tool".to_string());
        let mut detail = msg
            .interpreted_output
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                msg.raw_output
                    .clone()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_else(|| "无可用执行输出".to_string());
        if detail.len() > 280 {
            detail.truncate(280);
            detail.push_str("...");
        }

        let event_body = match msg.phase.as_str() {
            "mcp_result" => {
                let success = msg.success.unwrap_or(false);
                let executed = msg.executed.unwrap_or(false);
                let should_retry = msg.should_retry.unwrap_or(false);
                if success {
                    format!(
                        "任务分析结果: 工具={}, 执行成功={}, 需重试={}, 结论={} ",
                        tool_id, executed, should_retry, detail
                    )
                } else {
                    let reason = msg
                        .failure_reason
                        .clone()
                        .unwrap_or_else(|| "未知失败".to_string());
                    format!(
                        "任务分析结果: 工具={}, 执行成功=false, 需重试={}, 失败原因={}, 结论={}",
                        tool_id, should_retry, reason, detail
                    )
                }
            }
            "mcp_error" | "mcp_mailbox_error" | "mcp_timeout" => {
                let reason = msg
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| "未知错误".to_string());
                let retry_note = msg
                    .retry_in_secs
                    .map(|v| format!("，{}秒后重试", v))
                    .unwrap_or_default();
                format!(
                    "任务分析结果: 阶段={}, 原因={}{}",
                    msg.phase, reason, retry_note
                )
            }
            _ => format!("任务分析结果: {}", detail),
        };

        let event_text = DagOrchestrActor::build_task_markdown_message(
            &format!("任务({})", msg.task_id),
            &format!("智能体({})", msg.agent_id),
            "任务执行事件",
            &event_body,
        );

        let message = UserMessage {
            sender: "任务系统".to_string(),
            source_ip: "system".to_string(),
            device_type: "agent".to_string(),
            content: MessageContent::Text(event_text),
            created_at: chrono::Utc::now(),
        };

        self.channel_manager_actor
            .do_send(SaveMessage { message });
    }
}
