use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, ResponseActFuture,
    WrapFuture,
};
use uuid::Uuid;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::time::Duration;
use std::time::Instant;
use tokio::time::timeout;
use tracing::{error, info};

use crate::{
    chat::openai_actor::OpenAIProxyActor,
    mcp::mcp_actor::{ExecuteMcp, McpAgentActor},
    task::dag_orchestrator::{DagOrchestrActor, NotifyTaskExecutionEvent},
    workspace::model::AgentId,
};

impl AgentActor {
    fn start_running_loop(&self, ctx: &mut Context<Self>) {
        ctx.run_interval(
            Duration::from_secs(self.running_loop_interval_secs),
            |actor, ctx| {
                if actor.lifecycle != ActorLifecycle::Running {
                    return;
                }

                // 进入 Running 后的循环工作：先拉起任务检查。
                ctx.notify(RunAssignedTaskCheck);
            },
        );
    }
}

pub struct AgentActor {
    pub id: AgentId,
    pub lifecycle: ActorLifecycle,
    pub task_id: Option<Uuid>, // 任务id
    pub mcp_inflight_task_id: Option<Uuid>,
    pub mcp_last_failed_task_id: Option<Uuid>,
    pub mcp_next_retry_at: Option<Instant>,
    pub mcp_consecutive_failures: u32,
    pub mcp_exec_timeout_secs: u64,
    pub running_loop_interval_secs: u64,
    pub open_aiproxy_actor: Addr<OpenAIProxyActor>,
    pub mcp_agent_actor: Addr<McpAgentActor>,
    pub dag_orchestr_actor: Addr<DagOrchestrActor>,
    pub pool: sqlx::PgPool,
}
// 触发 Agent 立即检查分配的任务并更新状态（用于手动触发）
#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct RunAssignedTaskCheck;

/// Actor 生命周期快照（用于查询 Actor 本身的生命周期/连通性）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActorLifecycle {
    /// Actor 正在启动（已分配但尚未进入运行）
    Starting,
    /// Actor 正常运行
    Running,
    /// Actor 正在停止（进入停止流程）
    Stopping,
    /// Actor 已停止
    Stopped,
}

impl Actor for AgentActor {
    type Context = Context<Self>;

    // 启动
    fn started(&mut self, ctx: &mut Self::Context) {
        self.lifecycle = ActorLifecycle::Starting;
        info!("AgentActor [{}] started", self.id);

        let pool = self.pool.clone();
        let agent_id = self.id;

        ctx.spawn(
            async move {
                let row = sqlx::query(
                    "SELECT id, status FROM tasks WHERE assigned_agent_id = $1 AND status IN ('executing', 'accepted') ORDER BY CASE WHEN status = 'executing' THEN 0 ELSE 1 END, created_at ASC LIMIT 1",
                )
                .bind(agent_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| format!("query startup task failed: {}", e))?;

                let Some(row) = row else {
                    return Ok::<Option<(Uuid, String)>, String>(None);
                };

                let task_id = row
                    .try_get::<Uuid, _>("id")
                    .map_err(|e| format!("read startup task id failed: {}", e))?;
                let status = row
                    .try_get::<String, _>("status")
                    .map_err(|e| format!("read startup task status failed: {}", e))?;

                if status == "accepted" {
                    sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
                        .bind("executing")
                        .bind(task_id)
                        .execute(&pool)
                        .await
                        .map_err(|e| format!("promote accepted task to executing failed: {}", e))?;
                }

                Ok(Some((task_id, status)))
            }
            .into_actor(self)
            .map(|res: Result<Option<(Uuid, String)>, String>, actor, ctx| {
                match res {
                    Ok(Some((task_id, status))) => {
                        actor.task_id = Some(task_id);
                        if status == "executing" {
                            info!("AgentActor [{}] resumed executing task [{}]", actor.id, task_id);
                        } else {
                            info!(
                                "AgentActor [{}] promoted accepted task [{}] to executing on startup",
                                actor.id, task_id
                            );
                        }
                    }
                    Ok(None) => {
                        actor.task_id = None;
                    }
                    Err(e) => {
                        error!(
                            "AgentActor [{}] failed to restore startup task: {}",
                            actor.id, e
                        );
                        actor.task_id = None;
                    }
                }

                actor.lifecycle = ActorLifecycle::Running;
                info!("AgentActor [{}] lifecycle set to running", actor.id);
                actor.start_running_loop(ctx);
            }),
        );
    }

    // 停止
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.lifecycle = ActorLifecycle::Stopped;
        info!("AgentActor [{}] stopped", self.id);
    }
}

// 生命周期查询消息：返回 ActorLifecycle 快照
#[derive(Message)]
#[rtype(result = "Result<ActorLifecycle, ()>")]
pub struct GetLifecycle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeStatus {
    pub lifecycle: ActorLifecycle,
    pub task_id: Option<Uuid>,
    pub mcp_inflight_task_id: Option<Uuid>,
}

#[derive(Message)]
#[rtype(result = "Result<AgentRuntimeStatus, ()>")]
pub struct GetRuntimeStatus;

impl Handler<GetLifecycle> for AgentActor {
    type Result = Result<ActorLifecycle, ()>;
    fn handle(&mut self, _msg: GetLifecycle, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.lifecycle.clone())
    }
}

impl Handler<GetRuntimeStatus> for AgentActor {
    type Result = Result<AgentRuntimeStatus, ()>;

    fn handle(&mut self, _msg: GetRuntimeStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(AgentRuntimeStatus {
            lifecycle: self.lifecycle.clone(),
            task_id: self.task_id,
            mcp_inflight_task_id: self.mcp_inflight_task_id,
        })
    }
}

impl Handler<RunAssignedTaskCheck> for AgentActor {
    type Result = ResponseActFuture<Self, Result<(), ()>>;

    fn handle(&mut self, _msg: RunAssignedTaskCheck, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let agent_id = self.id;

        let work = async move {
            let row = sqlx::query(
                "SELECT id, status FROM tasks WHERE assigned_agent_id = $1 AND status IN ('accepted','executing') ORDER BY CASE WHEN status = 'executing' THEN 0 ELSE 1 END, created_at ASC LIMIT 1",
            )
            .bind(agent_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to query assigned tasks for agent {}: {}", agent_id, e);
                e
            })?;

            let Some(row) = row else {
                // 无分配任务时，主动抢占同 workspace 下的 published 无主任务
                let claim_row = sqlx::query(
                    r#"
                    UPDATE tasks
                    SET assigned_agent_id = $1, status = 'executing'
                    WHERE id = (
                        SELECT t.id
                        FROM tasks t
                        JOIN agents a ON a.workspace_name = t.workspace_name
                        WHERE a.id = $1
                          AND t.status = 'published'
                          AND t.assigned_agent_id IS NULL
                        ORDER BY t.created_at ASC
                        LIMIT 1
                    )
                    AND status = 'published'
                    AND assigned_agent_id IS NULL
                    RETURNING id
                    "#,
                )
                .bind(agent_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| {
                    tracing::error!(
                        "Agent {} failed to auto-claim published task: {}",
                        agent_id,
                        e
                    );
                    e
                })?;

                if let Some(claim_row) = claim_row {
                    let task_id = claim_row.try_get::<Uuid, _>("id").map_err(|e| {
                        tracing::error!(
                            "Agent {} failed to read auto-claimed task id: {}",
                            agent_id,
                            e
                        );
                        e
                    })?;
                    tracing::info!("Agent {} auto-claimed published task {}", agent_id, task_id);
                    return Ok(Some((task_id, "executing".to_string())));
                }

                return Ok(None);
            };

            let task_id = row.try_get::<Uuid, _>("id").map_err(|e| {
                tracing::error!(
                    "Failed to read assigned task id for agent {}: {}",
                    agent_id,
                    e
                );
                e
            })?;
            let status = row.try_get::<String, _>("status").map_err(|e| {
                tracing::error!(
                    "Failed to read assigned task status for agent {}: {}",
                    agent_id,
                    e
                );
                e
            })?;

            if status == "accepted" {
                sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
                    .bind("executing")
                    .bind(task_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to update task {} to executing: {}", task_id, e);
                        e
                    })?;
            }

            Ok(Some((task_id, status)))
        };

        Box::pin(
            work.into_actor(self)
                .map(|res: Result<Option<(Uuid, String)>, sqlx::Error>, actor, ctx| {
                    match res {
                        Ok(Some((task_id, status))) => {
                            let previous_task_id = actor.task_id;
                            actor.task_id = Some(task_id);

                            // 切换到新任务时，清理上一个任务的失败退避状态。
                            if previous_task_id != Some(task_id) {
                                actor.mcp_last_failed_task_id = None;
                                actor.mcp_next_retry_at = None;
                                actor.mcp_consecutive_failures = 0;
                            }

                            if status == "accepted" {
                                if previous_task_id != Some(task_id) {
                                    tracing::info!("Agent {} started task {}", actor.id, task_id);
                                }
                            } else if previous_task_id != Some(task_id) {
                                tracing::info!(
                                    "Agent {} resuming executing task {}",
                                    actor.id, task_id
                                );
                            }

                            // 仅在当前任务未在执行中时触发 MCP 执行，避免 run_interval 重复并发。
                            if actor.mcp_inflight_task_id != Some(task_id) {
                                // 若该任务处于退避窗口内，则暂不触发执行。
                                if actor.mcp_last_failed_task_id == Some(task_id) {
                                    if let Some(next_retry_at) = actor.mcp_next_retry_at {
                                        if Instant::now() < next_retry_at {
                                            return Ok(());
                                        }
                                    }
                                }

                                actor.mcp_inflight_task_id = Some(task_id);
                                let mcp_addr = actor.mcp_agent_actor.clone();
                                let pool = actor.pool.clone();
                                let agent_id = actor.id;
                                let mcp_exec_timeout_secs = actor.mcp_exec_timeout_secs;

                                ctx.spawn(
                                    async move {
                                        let exec_res = timeout(
                                            Duration::from_secs(mcp_exec_timeout_secs),
                                            mcp_addr.send(ExecuteMcp {
                                                agent_id,
                                                task_id: task_id.to_string(),
                                            }),
                                        )
                                        .await;

                                        let next_status: Option<&'static str> = match &exec_res {
                                            Ok(Ok(Ok(result))) => {
                                                if result.success {
                                                    Some("submitted")
                                                } else if result.should_retry {
                                                    None
                                                } else {
                                                    Some("completed_failure")
                                                }
                                            }
                                            Ok(Ok(Err(_))) | Ok(Err(_)) => Some("completed_failure"),
                                            Err(_) => None,
                                        };

                                        if let Some(status) = next_status {
                                            if let Err(e) = sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
                                                .bind(status)
                                                .bind(task_id)
                                                .execute(&pool)
                                                .await
                                            {
                                                tracing::error!(
                                                    "Agent {} failed to update task {} status to {}: {}",
                                                    agent_id,
                                                    task_id,
                                                    status,
                                                    e
                                                );
                                            }
                                        }

                                        (task_id, exec_res)
                                    }
                                    .into_actor(actor)
                                    .map(|(task_id, exec_res), actor, _ctx| {
                                        actor.mcp_inflight_task_id = None;
                                        match exec_res {
                                            Ok(Ok(Ok(result))) => {
                                                if result.success {
                                                    actor.mcp_last_failed_task_id = None;
                                                    actor.mcp_next_retry_at = None;
                                                    actor.mcp_consecutive_failures = 0;
                                                } else {
                                                    actor.mcp_last_failed_task_id = Some(task_id);
                                                    actor.mcp_consecutive_failures = actor.mcp_consecutive_failures.saturating_add(1);
                                                    let exp = actor.mcp_consecutive_failures.saturating_sub(1).min(5);
                                                    let delay_secs = (actor.running_loop_interval_secs.max(1)) * (1u64 << exp);
                                                    actor.mcp_next_retry_at = Some(Instant::now() + Duration::from_secs(delay_secs));
                                                }

                                                if result.success || !result.should_retry {
                                                    actor.task_id = None;
                                                }

                                                tracing::info!(
                                                    "Agent {} MCP finished for task {}: success={}, executed={}, should_retry={}, retry_failures={}",
                                                    actor.id,
                                                    task_id,
                                                    result.success,
                                                    result.executed,
                                                    result.should_retry,
                                                    actor.mcp_consecutive_failures
                                                );

                                                actor.dag_orchestr_actor.do_send(NotifyTaskExecutionEvent {
                                                    task_id,
                                                    agent_id: actor.id,
                                                    phase: "mcp_result".to_string(),
                                                    success: Some(result.success),
                                                    executed: Some(result.executed),
                                                    should_retry: Some(result.should_retry),
                                                    selected_tool_id: result.selected_tool_id.clone(),
                                                    interpreted_output: Some(result.interpreted_output.clone()),
                                                    raw_output: Some(result.raw_output.clone()),
                                                    failure_reason: result.failure_reason.clone(),
                                                    retry_in_secs: None,
                                                });
                                            }
                                            Ok(Ok(Err(e))) => {
                                                actor.mcp_last_failed_task_id = Some(task_id);
                                                actor.mcp_consecutive_failures = actor.mcp_consecutive_failures.saturating_add(1);
                                                let exp = actor.mcp_consecutive_failures.saturating_sub(1).min(5);
                                                let delay_secs = (actor.running_loop_interval_secs.max(1)) * (1u64 << exp);
                                                actor.mcp_next_retry_at = Some(Instant::now() + Duration::from_secs(delay_secs));

                                                tracing::error!(
                                                    "Agent {} MCP execution failed for task {}: {}, retry_in={}s",
                                                    actor.id,
                                                    task_id,
                                                    e,
                                                    delay_secs
                                                );

                                                actor.dag_orchestr_actor.do_send(NotifyTaskExecutionEvent {
                                                    task_id,
                                                    agent_id: actor.id,
                                                    phase: "mcp_error".to_string(),
                                                    success: None,
                                                    executed: None,
                                                    should_retry: Some(true),
                                                    selected_tool_id: None,
                                                    interpreted_output: None,
                                                    raw_output: None,
                                                    failure_reason: Some(e.to_string()),
                                                    retry_in_secs: Some(delay_secs),
                                                });
                                            }
                                            Ok(Err(e)) => {
                                                actor.mcp_last_failed_task_id = Some(task_id);
                                                actor.mcp_consecutive_failures = actor.mcp_consecutive_failures.saturating_add(1);
                                                let exp = actor.mcp_consecutive_failures.saturating_sub(1).min(5);
                                                let delay_secs = (actor.running_loop_interval_secs.max(1)) * (1u64 << exp);
                                                actor.mcp_next_retry_at = Some(Instant::now() + Duration::from_secs(delay_secs));

                                                tracing::error!(
                                                    "Agent {} MCP mailbox error for task {}: {}, retry_in={}s",
                                                    actor.id,
                                                    task_id,
                                                    e,
                                                    delay_secs
                                                );

                                                actor.dag_orchestr_actor.do_send(NotifyTaskExecutionEvent {
                                                    task_id,
                                                    agent_id: actor.id,
                                                    phase: "mcp_mailbox_error".to_string(),
                                                    success: None,
                                                    executed: None,
                                                    should_retry: Some(true),
                                                    selected_tool_id: None,
                                                    interpreted_output: None,
                                                    raw_output: None,
                                                    failure_reason: Some(e.to_string()),
                                                    retry_in_secs: Some(delay_secs),
                                                });
                                            }
                                            Err(e) => {
                                                actor.mcp_last_failed_task_id = Some(task_id);
                                                actor.mcp_consecutive_failures = actor.mcp_consecutive_failures.saturating_add(1);
                                                let exp = actor.mcp_consecutive_failures.saturating_sub(1).min(5);
                                                let delay_secs = (actor.running_loop_interval_secs.max(1)) * (1u64 << exp);
                                                actor.mcp_next_retry_at = Some(Instant::now() + Duration::from_secs(delay_secs));

                                                tracing::error!(
                                                    "Agent {} MCP timed out for task {}: {}, retry_in={}s",
                                                    actor.id,
                                                    task_id,
                                                    e,
                                                    delay_secs
                                                );

                                                actor.dag_orchestr_actor.do_send(NotifyTaskExecutionEvent {
                                                    task_id,
                                                    agent_id: actor.id,
                                                    phase: "mcp_timeout".to_string(),
                                                    success: None,
                                                    executed: None,
                                                    should_retry: Some(true),
                                                    selected_tool_id: None,
                                                    interpreted_output: None,
                                                    raw_output: None,
                                                    failure_reason: Some(e.to_string()),
                                                    retry_in_secs: Some(delay_secs),
                                                });
                                            }
                                        }
                                    }),
                                );
                            }
                        }
                        Ok(None) => {
                            actor.task_id = None;
                            actor.mcp_inflight_task_id = None;
                        }
                        Err(_) => {
                            actor.task_id = None;
                            actor.mcp_inflight_task_id = None;
                        }
                    }
                    Ok(())
                }),
        )
    }
}
