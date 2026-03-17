use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, ResponseActFuture,
    WrapFuture,
};
use uuid::Uuid;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::time::Duration;
use tracing::{error, info};

use crate::{
    chat::openai_actor::OpenAIProxyActor,
    mcp::mcp_actor::McpAgentActor,
    task::dag_orchestrator::DagOrchestrActor,
    workspace::model::AgentId,
};

impl AgentActor {
    fn start_running_loop(&self, ctx: &mut Context<Self>) {
        ctx.run_interval(Duration::from_secs(self.running_loop_interval_secs), |actor, ctx| {
            if actor.lifecycle != ActorLifecycle::Running {
                return;
            }

            // 进入 Running 后的循环工作：先拉起任务检查。
            ctx.notify(RunAssignedTaskCheck);

            // 预留 mcp_actor 接入点，后续可在这里触发 MCP tool 执行。
            let _mcp_agent_actor = actor.mcp_agent_actor.clone();
            // 预留openai_proxy_actor 接入点，后续可在这里触发 LLM 相关的操作。
            let _open_aiproxy_actor = actor.open_aiproxy_actor.clone();
        });
    }
}

pub struct AgentActor {
    pub id: AgentId,
    pub lifecycle: ActorLifecycle,
    pub task_id: Option<Uuid>, // 任务id
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

impl Handler<GetLifecycle> for AgentActor {
    type Result = Result<ActorLifecycle, ()>;
    fn handle(&mut self, _msg: GetLifecycle, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.lifecycle.clone())
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
                return Ok(None);
            };

            let task_id = row.try_get::<Uuid, _>("id").map_err(|e| {
                tracing::error!("Failed to read assigned task id for agent {}: {}", agent_id, e);
                e
            })?;
            let status = row.try_get::<String, _>("status").map_err(|e| {
                tracing::error!("Failed to read assigned task status for agent {}: {}", agent_id, e);
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
                .map(|res: Result<Option<(Uuid, String)>, sqlx::Error>, actor, _ctx| {
                    match res {
                        Ok(Some((task_id, status))) => {
                            let previous_task_id = actor.task_id;
                            actor.task_id = Some(task_id);

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
                        }
                        Ok(None) => {
                            actor.task_id = None;
                        }
                        Err(_) => {
                            actor.task_id = None;
                        }
                    }
                    Ok(())
                }),
        )
    }
}
