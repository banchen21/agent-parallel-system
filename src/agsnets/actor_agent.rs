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
    mcp::mcp_actor::{ExecuteMcp, McpAgentActor},
    task::dag_orchestrator::{DagOrchestrator, QueryTaskById},
    workspace::model::AgentId,
};

pub struct AgentActor {
    pub id: AgentId,
    pub lifecycle: ActorLifecycle,
    pub task_id: Option<Uuid>, // 任务id
    pub open_aiproxy_actor: Addr<OpenAIProxyActor>,
    pub mcp_agent_actor: Addr<McpAgentActor>,
    pub dag_orchestrator: Addr<DagOrchestrator>,
    pub pool: sqlx::PgPool,
}

impl AgentActor {
    pub fn start_task_execution(&mut self) -> ResponseActFuture<Self, Result<(), ()>> {
        // 先检查 lifecycle，若不在 Running 则立即返回一个 actor-future
        let lifecycle = self.lifecycle.clone();
        if lifecycle != ActorLifecycle::Running {
            info!(
                "Agent {} is not in Running state, cannot start task execution",
                self.id
            );
            return Box::pin(async { Ok(()) }.into_actor(self));
        }

        // 必须有 task_id
        let task_id = match self.task_id {
            Some(id) => id,
            None => {
                info!("Agent {} has no assigned task", self.id);
                return Box::pin(async { Ok(()) }.into_actor(self));
            }
        };

        // 克隆要在 async move 中使用的资源
        let agent_id = self.id;
        let mcp_addr = self.mcp_agent_actor.clone();
        let openai = self.open_aiproxy_actor.clone();
        let dag = self.dag_orchestrator.clone();
        let pool = self.pool.clone();

        // 异步工作：注意所有错误都要转换为 `()`
        let work = async move {
            // 查询任务详情（示例），将所有错误转换为 `()`
            let task_res = dag
                .send(QueryTaskById(task_id))
                .await
                .map_err(|e| {
                    error!("Failed to send QueryTaskById message: {}", e);
                })?
                .map_err(|e| {
                    error!("Failed to query task by id {}: {}", task_id, e);
                })?;

            // 获取任务前置条件、参数等信息（示例占位，替换为实际字段）
            let task_info = task_res; // 假设 task_res 已经是我们需要
            let mcp_res = mcp_addr
                .send(ExecuteMcp {
                    agent_id,
                    task_id: task_id.to_string(),
                })
                .await
                .map_err(|e| {
                    error!("Failed to send ExecuteMcp message: {}", e);
                })?;
            // TODO: 在这里调用 MCP（示例占位，替换为实际消息类型与错误处理）
            // let mcp_res = mcp_addr.send(ExecuteMcp { agent_id: agent_id.to_string(), task_id: task_id.to_string() }).await;
            // match mcp_res {
            //     Ok(Ok(_)) => { /* started */ }
            //     _ => { tracing::error!(...); return Err(()); }
            // }

            // 其它执行逻辑...
            Ok(())
        };

        Box::pin(
            work.into_actor(self)
                .map(move |res: Result<(), ()>, actor, _ctx| {
                    if res.is_ok() {
                        actor.task_id = Some(task_id);
                        actor.lifecycle = ActorLifecycle::Running;
                    }
                    res
                }),
        )
    }
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
        info!("AgentActor {} started", self.id);

        // 周期性检查：查找分配给该 Agent 且处于 accepted 的任务并标记为 executing
        let interval = Duration::from_secs(5);
        ctx.run_interval(interval, |actor, ctx| {
            let pool = actor.pool.clone();
            let agent_id = actor.id;

            // 使用 actor-aware future 在异步 DB 操作完成后可以安全地访问并修改 actor 的状态
            let work = async move {
                match sqlx::query(
                    "SELECT id, status FROM tasks WHERE assigned_agent_id = $1 AND status IN ('accepted','executing')",
                )
                .bind(agent_id)
                .fetch_all(&pool)
                .await
                {
                    Ok(rows) => {
                        let mut any_started = false;
                        for row in rows {
                            // 读取 id 与 status
                            let task_id_res = row.try_get::<Uuid, _>("id");
                            let status_res = row.try_get::<String, _>("status");
                            if let (Ok(task_id), Ok(status)) = (task_id_res, status_res) {
                                match status.as_str() {
                                    "accepted" => {
                                        // 尝试从 accepted 切换到 executing
                                        match sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
                                            .bind("executing")
                                            .bind(task_id)
                                            .execute(&pool)
                                            .await
                                        {
                                            Ok(_) => {
                                                tracing::info!(
                                                    "Agent [{}] started task [{}]",
                                                    agent_id,
                                                    task_id
                                                );
                                                any_started = true;
                                            }
                                            Err(e) => tracing::error!(
                                                "Failed to update task [{}] to executing: {}",
                                                task_id,
                                                e
                                            ),
                                        }
                                    }
                                    "executing" => {
                                        // 任务已在执行中，视为需要继续运行
                                        tracing::info!("Agent [{}] resuming executing task [{}]", agent_id, task_id);
                                        any_started = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(any_started)
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to query assigned tasks for agent [{}]: {}",
                            agent_id,
                            e
                        );
                        Ok(false)
                    }
                }
            };

            // 将异步工作转换为 actor future，然后在回调中根据结果设置生命周期
            ctx.spawn(
                work.into_actor(actor)
                    .map(|res: Result<bool, sqlx::Error>, actor, _ctx| {
                        if let Ok(found) = res {
                            if found {
                                actor.lifecycle = ActorLifecycle::Running;
                            }
                        }
                    }),
            );
        });
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
            match sqlx::query(
                "SELECT id, status FROM tasks WHERE assigned_agent_id = $1 AND status IN ('accepted','executing')",
            )
            .bind(agent_id)
            .fetch_all(&pool)
            .await
            {
                Ok(rows) => {
                    let mut any_started = false;
                    for row in rows {
                        let task_id_res = row.try_get::<Uuid, _>("id");
                        let status_res = row.try_get::<String, _>("status");
                        if let (Ok(task_id), Ok(status)) = (task_id_res, status_res) {
                            match status.as_str() {
                                "accepted" => {
                                    if let Err(e) = sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
                                        .bind("executing")
                                        .bind(task_id)
                                        .execute(&pool)
                                        .await
                                    {
                                        tracing::error!("Failed to update task {} to executing: {}", task_id, e);
                                    } else {
                                        tracing::info!("Agent {} started task {}", agent_id, task_id);
                                        any_started = true;
                                    }
                                }
                                "executing" => {
                                    tracing::info!("Agent {} resuming executing task {}", agent_id, task_id);
                                    any_started = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(any_started)
                }
                Err(e) => {
                    tracing::error!("Failed to query assigned tasks for agent {}: {}", agent_id, e);
                    Ok(false)
                }
            }
        };

        Box::pin(
            work.into_actor(self)
                .map(|res: Result<bool, sqlx::Error>, _actor, _ctx| {
                    if let Ok(found) = res {
                        if found {
                            // found assigned tasks (already updated to executing or resumed)
                        }
                    }
                    Ok(())
                }),
        )
    }
}
