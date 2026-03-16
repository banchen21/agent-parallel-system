use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, ResponseActFuture,
    WrapFuture,
};
use uuid::Uuid;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info};

use crate::{
    agsnets::{
        actor_agent::{ActorLifecycle, AgentActor, GetLifecycle, RunAssignedTaskCheck},
        model::AgentError,
    },
    chat::openai_actor::OpenAIProxyActor,
    mcp::mcp_actor::McpAgentActor,
    task::dag_orchestrator::DagOrchestrator,
    workspace::model::{AgentId, AgentInfo, AgentKind},
};

pub struct AgentManagerActor {
    pool: sqlx::PgPool,
    agents: HashMap<AgentId, Addr<AgentActor>>,
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    mcp_manager: Addr<McpAgentActor>,
    dag_orchestrator: Addr<DagOrchestrator>,
}

impl AgentManagerActor {
    pub fn new(
        pool: sqlx::PgPool,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        mcp_manager: Addr<McpAgentActor>,
        dag_orchestrator: Addr<DagOrchestrator>,
    ) -> Self {
        let this = AgentManagerActor {
            agents: HashMap::new(),
            open_aiproxy_actor,
            mcp_manager,
            dag_orchestrator,
            pool,
        };
        this
    }

    pub fn spawn_auto_scan_loop(addr: Addr<AgentManagerActor>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                let _ = addr.send(AutoScan {}).await;
            }
        });
    }
}

impl Actor for AgentManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("AgentManageActor started");
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
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

                let kind_str = msg.kind.as_db_str();

                let res = sqlx::query(
                    "INSERT INTO agents (id, name, kind, workspace_name, owner_username) VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(id)
                .bind(&msg.name)
                .bind(kind_str)
                .bind(&msg.workspace_name)
                .bind(&msg.user_name)
                .execute(&pool)
                .await;

                match res {
                    Ok(_) => Ok(AgentInfo {
                        id,
                        name: msg.name,
                        kind: msg.kind,
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
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            mcp_agent_actor: self.mcp_manager.clone(),
            dag_orchestrator: self.dag_orchestrator.clone(),
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
        Box::pin(async move { Ok(()) }.into_actor(self))
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
                let res = sqlx::query("SELECT id, name, kind, workspace_name, owner_username FROM agents WHERE owner_username = $1")
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
                            let workspace_name: String = row.try_get("workspace_name").map_err(AgentError::DatabaseError)?;
                            let owner_username: String = row.try_get("owner_username").map_err(AgentError::DatabaseError)?;

                            // 默认状态为 stopped（数据库存在但未在内存中运行）
                            let mut status = String::from("stopped");
                            if let Some(addr) = agents_map.get(&id) {
                                // 显式标注 await 的返回类型以帮助类型推断
                                let lifecycle_res: Result<Result<ActorLifecycle, ()>, actix::MailboxError> = addr.send(GetLifecycle {}).await;
                                match lifecycle_res {
                                    Ok(Ok(lc)) => {
                                        status = match lc {
                                            ActorLifecycle::Starting => String::from("starting"),
                                            ActorLifecycle::Running => String::from("running"),
                                            ActorLifecycle::Stopping => String::from("stopping"),
                                            ActorLifecycle::Stopped => String::from("stopped"),
                                        }
                                    }
                                    _ => status = String::from("unknown"),
                                }
                            }

                            list.push(AgentInfo {
                                id,
                                name,
                                kind: AgentKind::from_db_str(&kind_str),
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
                // 优先尝试在内存中的 AgentActor 中找到处于 Running 状态的实例
                for (id, addr) in agents_snapshot.into_iter() {
                    match addr.send(GetLifecycle {}).await {
                        Ok(Ok(lc)) => {
                                // 只把处于 Running 的 Actor 视为可用
                                if matches!(lc, ActorLifecycle::Running) {
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

// 自动扫描消息（由后台 loop 发送）
#[derive(Message)]
#[rtype(result = "()")]
pub struct AutoScan;

impl Handler<AutoScan> for AgentManagerActor {
    type Result = ResponseActFuture<Self, ()>;
    fn handle(&mut self, _msg: AutoScan, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        // 快照已存在的 agent id
        let existing_ids: Vec<AgentId> = self.agents.keys().cloned().collect();

        let open_aiproxy = self.open_aiproxy_actor.clone();
        let mcp = self.mcp_manager.clone();
        let dag = self.dag_orchestrator.clone();

        let work = async move {
            match sqlx::query("SELECT id FROM agents").fetch_all(&pool).await {
                Ok(rows) => rows
                    .into_iter()
                    .filter_map(|r| r.try_get::<Uuid, _>("id").ok())
                    .collect::<Vec<Uuid>>(),
                Err(e) => {
                    error!("Failed to fetch agents from db: {}", e);
                    Vec::new()
                }
            }
        };

        Box::pin(work.into_actor(self).map(move |ids, actor, _ctx| {
            for id in ids.into_iter() {
                if !existing_ids.contains(&id) {
                    let a = AgentActor {
                        id,
                        lifecycle: ActorLifecycle::Starting,
                        task_id: None,
                        open_aiproxy_actor: open_aiproxy.clone(),
                        mcp_agent_actor: mcp.clone(),
                        dag_orchestrator: dag.clone(),
                        pool: actor.pool.clone(),
                    };

                    let addr = a.start();
                    actor.agents.insert(id, addr);
                    info!("Auto-started AgentActor {}", id);
                }
            }
        }))
    }
}
