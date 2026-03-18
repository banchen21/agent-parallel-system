use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, ResponseActFuture,
    WrapFuture,
};
use uuid::Uuid;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use tracing::{error, info};

use crate::{
    agsnets::{
        actor_agent::{
            ActorLifecycle, AgentActor, GetRuntimeStatus, RunAssignedTaskCheck, ShutdownAgent,
        },
        model::AgentError,
    },
    chat::openai_actor::OpenAIProxyActor,
    mcp::mcp_actor::McpAgentActor,
    task::dag_orchestrator::DagOrchestrActor,
    utils::workspace_path::{ensure_dir, workspace_agents_dir},
    workspace::model::{AgentId, AgentInfo, AgentKind},
};

#[derive(Clone)]
pub struct AgentManagerActor {
    pub pool: sqlx::PgPool,
    pub agents: HashMap<AgentId, Addr<AgentActor>>,
    running_loop_interval_secs: u64,
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    mcp_manager: Addr<McpAgentActor>,
    dag_orchestrator: Addr<DagOrchestrActor>,
}

impl AgentManagerActor {
    pub fn new(
        pool: sqlx::PgPool,
        running_loop_interval_secs: u64,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        mcp_manager: Addr<McpAgentActor>,
        dag_orchestrator: Addr<DagOrchestrActor>,
    ) -> Self {
        let this = AgentManagerActor {
            agents: HashMap::new(),
            running_loop_interval_secs,
            open_aiproxy_actor,
            mcp_manager,
            dag_orchestrator,
            pool,
        };
        this
    }
}

impl Actor for AgentManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("AgentManageActor started");

        // 一次性启动扫描：读取表内全部 agents，并全部以 Starting 状态启动
        let pool = self.pool.clone();
        let open_ai = self.open_aiproxy_actor.clone();
        let mcp = self.mcp_manager.clone();
        let dag = self.dag_orchestrator.clone();

        let work = async move {
            let rows = sqlx::query(
                "SELECT id AS agent_id FROM agents ORDER BY id",
            )
            .fetch_all(&pool)
            .await;

            match rows {
                Ok(rows) => {
                    let mut to_start: Vec<Uuid> = Vec::new();
                    for row in &rows {
                        let agent_id: Uuid = row.try_get("agent_id").unwrap();
                        if !to_start.contains(&agent_id) {
                            to_start.push(agent_id );
                        }
                    }
                    Ok(to_start)
                }
                Err(e) => {
                    error!("Startup scan: failed to query agents: {}", e);
                    Ok(Vec::new())
                }
            }
        };

        ctx.spawn(
            work.into_actor(self).map(
                move |res: Result<Vec<Uuid>, sqlx::Error>, actor, _ctx| {
                    if let Ok(to_start) = res {
                        for agent_id in to_start {
                            let a = AgentActor {
                                id: agent_id,
                                lifecycle: ActorLifecycle::Starting,
                                task_id: None,
                                mcp_inflight_task_id: None,
                                mcp_last_failed_task_id: None,
                                mcp_next_retry_at: None,
                                mcp_consecutive_failures: 0,
                                mcp_exec_timeout_secs: 60,
                                running_loop_interval_secs: actor.running_loop_interval_secs,
                                open_aiproxy_actor: open_ai.clone(),
                                mcp_agent_actor: mcp.clone(),
                                dag_orchestr_actor: dag.clone(),
                                pool: actor.pool.clone(),
                            };
                            let addr = a.start();
                            actor.agents.insert(agent_id, addr);
                            info!("Startup: started AgentActor {} with lifecycle=starting", agent_id);
                        }
                        info!("Startup scan complete: {} agents loaded", actor.agents.len());
                    }
                },
            ),
        );
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("AgentManageActor stopped");
    }
}

// 创建 Agent
#[derive(Message, Clone, Deserialize, Serialize)]
#[rtype(result = "Result<AgentInfo, AgentError>")]
pub struct CreateAgent {
    pub user_name: String,
    pub name: String,
    pub kind: AgentKind,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    pub workspace_name: String,
    #[serde(default)]
    pub mcp_list: Vec<String>,
}

impl Handler<CreateAgent> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<AgentInfo, AgentError>>;
    fn handle(&mut self, msg: CreateAgent, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        Box::pin(
            async move {
                // 生成 UUID
                let id = Uuid::new_v4();

                // 在工作区目录下创建同名智能体目录：.workspaces/<workspace_name>/agents/<agent_name>/
                let agent_dir = workspace_agents_dir(&msg.workspace_name).join(&msg.name);
                ensure_dir(&agent_dir)?;

                let kind_str = msg.kind.as_db_str();
                let provider = if msg.provider.trim().is_empty() {
                    String::from("default")
                } else {
                    msg.provider.trim().to_string()
                };
                let model = msg.model.trim().to_string();

                let res = sqlx::query(
                    "INSERT INTO agents (id, name, kind, provider, model, workspace_name, owner_username) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(id)
                .bind(&msg.name)
                .bind(kind_str)
                .bind(&provider)
                .bind(&model)
                .bind(&msg.workspace_name)
                .bind(&msg.user_name)
                .execute(&pool)
                .await;

                match res {
                    Ok(_) => Ok(AgentInfo {
                        id,
                        name: msg.name,
                        kind: msg.kind,
                        provider,
                        model,
                        workspace_name: msg.workspace_name,
                        owner_username: msg.user_name,
                        status: String::from("stopped"),
                        mcp_list: msg.mcp_list,
                    }),
                    Err(e) => Err(AgentError::DatabaseError(e)),
                }
            }
            .into_actor(self),
        )
    }
}

// 启动Agent
#[derive(Message)]
#[rtype(result = "Result<(), AgentError>")]
pub struct StartAgent {
    pub agent_id: AgentId,
}

impl Handler<StartAgent> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<(), AgentError>>;
    fn handle(&mut self, msg: StartAgent, _ctx: &mut Self::Context) -> Self::Result {
        // 如果该 agent 已经存在于内存中，直接返回
        if self.agents.contains_key(&msg.agent_id) {
            return Box::pin(async move { Ok(()) }.into_actor(self));
        }

        // 创建并启动 AgentActor 实例，然后将其 Addr 存入管理器
        let actor = AgentActor {
            id: msg.agent_id,
            lifecycle: ActorLifecycle::Starting,
            task_id: None,
            mcp_inflight_task_id: None,
            mcp_last_failed_task_id: None,
            mcp_next_retry_at: None,
            mcp_consecutive_failures: 0,
            mcp_exec_timeout_secs: 60,
            running_loop_interval_secs: self.running_loop_interval_secs,
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            mcp_agent_actor: self.mcp_manager.clone(),
            dag_orchestr_actor: self.dag_orchestrator.clone(),
            pool: self.pool.clone(),
        };

        let addr = actor.start();
        self.agents.insert(msg.agent_id, addr);

        Box::pin(async move { Ok(()) }.into_actor(self))
    }
}

// 停止Agent
#[derive(Message)]
#[rtype(result = "Result<(), AgentError>")]
pub struct StopAgent {
    pub agent_id: AgentId,
}

impl Handler<StopAgent> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<(), AgentError>>;
    fn handle(&mut self, msg: StopAgent, _ctx: &mut Self::Context) -> Self::Result {
        let addr_opt = self.agents.remove(&msg.agent_id);
        Box::pin(
            async move {
                if let Some(addr) = addr_opt {
                    let _ = addr.send(ShutdownAgent).await;
                }
                Ok(())
            }
            .into_actor(self),
        )
    }
}

// 删除Agent
#[derive(Message)]
#[rtype(result = "Result<(), AgentError>")]
pub struct DeleteAgent {
    pub agent_id: AgentId,
    pub user_name: String,
}

impl Handler<DeleteAgent> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<(), AgentError>>;

    fn handle(&mut self, msg: DeleteAgent, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let addr_opt = self.agents.remove(&msg.agent_id);

        Box::pin(
            async move {
                if let Some(addr) = addr_opt {
                    let _ = addr.send(ShutdownAgent).await;
                }

                let result = sqlx::query("DELETE FROM agents WHERE id = $1 AND owner_username = $2")
                    .bind(msg.agent_id)
                    .bind(msg.user_name)
                    .execute(&pool)
                    .await
                    .map_err(AgentError::DatabaseError)?;

                if result.rows_affected() == 0 {
                    return Err(AgentError::Message("agent not found or no permission".into()));
                }

                Ok(())
            }
            .into_actor(self),
        )
    }
}

// 获取user的agent列表
#[derive(Message)]
#[rtype(result = "Result<Vec<AgentInfo>, AgentError>")]
pub struct ListAgents {
    pub user_name: String,
}

impl Handler<ListAgents> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<Vec<AgentInfo>, AgentError>>;
    fn handle(&mut self, msg: ListAgents, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        // 快照当前内存中的 Agent addr，用于在异步查询中获取生命周期
        let agents_map = self.agents.clone();

        Box::pin(
            async move {
                let res = sqlx::query("SELECT id, name, kind, provider, model, workspace_name, owner_username FROM agents WHERE owner_username = $1")
                    .bind(&msg.user_name)
                    .fetch_all(&pool)
                    .await;

                match res {
                    Ok(rows) => {
                        let mut list: Vec<AgentInfo> = Vec::new();
                        for row in rows {
                            let id: Uuid = row.try_get("id").map_err(AgentError::DatabaseError)?;
                            let name: String = row.try_get("name").map_err(AgentError::DatabaseError)?;
                            let kind_str: String = row.try_get("kind").map_err(AgentError::DatabaseError)?;
                            let provider: String = row.try_get("provider").map_err(AgentError::DatabaseError)?;
                            let model: String = row.try_get("model").map_err(AgentError::DatabaseError)?;
                            let workspace_name: String = row.try_get("workspace_name").map_err(AgentError::DatabaseError)?;
                            let owner_username: String = row.try_get("owner_username").map_err(AgentError::DatabaseError)?;

                            // 默认状态为 stopped（数据库存在但未在内存中运行）
                            let mut status = String::from("stopped");
                            if let Some(addr) = agents_map.get(&id) {
                                let runtime_res = addr.send(GetRuntimeStatus {}).await;
                                match runtime_res {
                                    Ok(Ok(runtime)) => {
                                        status = match runtime.lifecycle {
                                            ActorLifecycle::Starting => String::from("starting"),
                                            ActorLifecycle::Stopping => String::from("stopping"),
                                            ActorLifecycle::Stopped => String::from("stopped"),
                                            ActorLifecycle::Running => {
                                                if runtime.task_id.is_some() || runtime.mcp_inflight_task_id.is_some() {
                                                    String::from("working")
                                                } else {
                                                    String::from("idle")
                                                }
                                            }
                                        }
                                    }
                                    _ => status = String::from("unknown"),
                                }
                            }

                            list.push(AgentInfo {
                                id,
                                name,
                                kind: AgentKind::from_db_str(&kind_str),
                                provider,
                                model,
                                workspace_name,
                                owner_username,
                                status,
                                mcp_list: vec![],
                            });
                        }
                        Ok(list)
                    }
                    Err(e) => Err(AgentError::DatabaseError(e)),
                }
            }
            .into_actor(self),
        )
    }
}

#[derive(Message)]
#[rtype(result = "Result<AgentInfo, AgentError>")]
pub struct GetAgentInfo {
    pub agent_id: AgentId,
    pub user_name: String,
}

impl Handler<GetAgentInfo> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<AgentInfo, AgentError>>;

    fn handle(&mut self, msg: GetAgentInfo, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let agents_map = self.agents.clone();

        Box::pin(
            async move {
                let row = sqlx::query(
                    "SELECT id, name, kind, provider, model, workspace_name, owner_username FROM agents WHERE id = $1 AND owner_username = $2",
                )
                .bind(msg.agent_id)
                .bind(&msg.user_name)
                .fetch_optional(&pool)
                .await
                .map_err(AgentError::DatabaseError)?;

                let row = match row {
                    Some(row) => row,
                    None => return Err(AgentError::Message("agent not found or no permission".into())),
                };

                let id: Uuid = row.try_get("id").map_err(AgentError::DatabaseError)?;
                let name: String = row.try_get("name").map_err(AgentError::DatabaseError)?;
                let kind_str: String = row.try_get("kind").map_err(AgentError::DatabaseError)?;
                let provider: String = row.try_get("provider").map_err(AgentError::DatabaseError)?;
                let model: String = row.try_get("model").map_err(AgentError::DatabaseError)?;
                let workspace_name: String = row.try_get("workspace_name").map_err(AgentError::DatabaseError)?;
                let owner_username: String = row.try_get("owner_username").map_err(AgentError::DatabaseError)?;

                let mut status = String::from("stopped");
                if let Some(addr) = agents_map.get(&id) {
                    match addr.send(GetRuntimeStatus {}).await {
                        Ok(Ok(runtime)) => {
                            status = match runtime.lifecycle {
                                ActorLifecycle::Starting => String::from("starting"),
                                ActorLifecycle::Stopping => String::from("stopping"),
                                ActorLifecycle::Stopped => String::from("stopped"),
                                ActorLifecycle::Running => {
                                    if runtime.task_id.is_some() || runtime.mcp_inflight_task_id.is_some() {
                                        String::from("working")
                                    } else {
                                        String::from("idle")
                                    }
                                }
                            }
                        }
                        _ => status = String::from("unknown"),
                    }
                }

                Ok(AgentInfo {
                    id,
                    name,
                    kind: AgentKind::from_db_str(&kind_str),
                    provider,
                    model,
                    workspace_name,
                    owner_username,
                    status,
                    mcp_list: vec![],
                })
            }
            .into_actor(self),
        )
    }
}

// 获取空闲的Agent
#[derive(Message)]
#[rtype(result = "Result<AgentId, AgentError>")]
pub struct CheckAvailableAgent {
    // 工作空间
    pub workspace_name: String,
    pub user_name: String,
}

// 触发指定 Agent 立即检查并启动分配给它的任务
#[derive(Message)]
#[rtype(result = "Result<(), AgentError>")]
pub struct TriggerAgentPoll {
    pub agent_id: AgentId,
}

impl Handler<CheckAvailableAgent> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<AgentId, AgentError>>;
    fn handle(&mut self, _msg: CheckAvailableAgent, _ctx: &mut Self::Context) -> Self::Result {
        // 在同步上下文中快照当前 agents 的 id/addr 列表，避免在 async move 中捕获 `self`
        let agents_snapshot: Vec<(AgentId, Addr<AgentActor>)> = self
            .agents
            .iter()
            .map(|(id, addr)| (id.clone(), addr.clone()))
            .collect();

        let pool = self.pool.clone();

        Box::pin(
            async move {
                // 优先尝试在内存中的 AgentActor 中找到真正空闲的实例
                for (id, addr) in agents_snapshot.into_iter() {
                    match addr.send(GetRuntimeStatus {}).await {
                        Ok(Ok(runtime)) => {
                            if matches!(runtime.lifecycle, ActorLifecycle::Running)
                                && runtime.task_id.is_none()
                                && runtime.mcp_inflight_task_id.is_none()
                            {
                                return Ok(id);
                            }
                        }
                        _ => continue,
                    }
                }

                // 回退：从数据库中挑选一个未指定的 agent（按 workspace + owner 匹配）
                let db_res = sqlx::query("SELECT id FROM agents WHERE workspace_name = $1 AND owner_username = $2 LIMIT 1")
                    .bind(&_msg.workspace_name)
                    .bind(&_msg.user_name)
                    .fetch_optional(&pool)
                    .await;

                match db_res {
                    Ok(Some(row)) => {
                        let id: Uuid = row.try_get("id").map_err(AgentError::DatabaseError)?;
                        Ok(id)
                    }
                    Ok(None) => Err(AgentError::Message("no available agent".into())),
                    Err(e) => Err(AgentError::DatabaseError(e)),
                }
            }
            .into_actor(self),
        )
    }
}

impl Handler<TriggerAgentPoll> for AgentManagerActor {
    type Result = ResponseActFuture<Self, Result<(), AgentError>>;

    fn handle(&mut self, msg: TriggerAgentPoll, _ctx: &mut Self::Context) -> Self::Result {
        let addr_opt = self.agents.get(&msg.agent_id).cloned();
        Box::pin(
            async move {
                if let Some(addr) = addr_opt {
                    // 触发 AgentActor 执行一次立即检查
                    let _ = addr.send(RunAssignedTaskCheck {}).await;
                    Ok(())
                } else {
                    Err(AgentError::Message("Agent not running".into()))
                }
            }
            .into_actor(self),
        )
    }
}
