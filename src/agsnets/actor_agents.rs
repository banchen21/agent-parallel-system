use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, ResponseActFuture,
    ResponseFuture,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use tracing::{error, info};

use crate::{
    agsnets::model::AgentError,
    chat::openai_actor::OpenAIProxyActor,
    mcp::mcp_actor::{ExecuteMcpTool, McpManagerActor},
    task_handler::actor_task::{DagOrchestrator, RegisterAgent},
    utils::workspace_path::{agent_dir, agent_memory_dir, ensure_dir},
    workspace::model::{AgentId, AgentInfo, AgentKind},
};

// ======================== 状态类型 ========================

/// 智能体三态（对外暴露，用于展示）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// 在线：已注册并运行，尚未接收任务
    Online,
    /// 工作中：正在处理任务
    Working,
    /// 空闲：任务已完成，等待下一个任务
    Idle,
}

impl AgentStatus {
    /// 返回中文状态标签
    pub fn label(&self) -> &'static str {
        match self {
            AgentStatus::Online => "在线",
            AgentStatus::Working => "工作中",
            AgentStatus::Idle => "空闲",
        }
    }
}

/// AgentActor 的状态快照（用于 HTTP 响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusInfo {
    pub agent_id: AgentId,
    pub name: String,
    pub status: AgentStatus,
    pub status_label: String,
}

// ======================== AgentManageActor ========================

pub struct AgentManageActor {
    agents: HashMap<AgentId, Addr<AgentActor>>,
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    mcp_manager: Addr<McpManagerActor>,
    dag_orchestrator: Addr<DagOrchestrator>,
    pool: sqlx::PgPool,
}

impl AgentManageActor {
    pub fn new(
        pool: sqlx::PgPool,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        mcp_manager: Addr<McpManagerActor>,
        dag_orchestrator: Addr<DagOrchestrator>,
    ) -> Self {
        Self {
            agents: HashMap::new(),
            open_aiproxy_actor,
            mcp_manager,
            dag_orchestrator,
            pool,
        }
    }
}

impl Actor for AgentManageActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("AgentManageActor 已启动，开始恢复已存在的 Agent");

        let pool = self.pool.clone();
        let open_aiproxy_actor = self.open_aiproxy_actor.clone();
        let mcp_manager = self.mcp_manager.clone();

        ctx.spawn(
            actix::fut::wrap_future(async move {
                sqlx::query(
                    r#"
                    SELECT id, name, kind, workspace_name, owner_username, mcp_list
                    FROM agents
                    ORDER BY name ASC
                    "#,
                )
                .fetch_all(&pool)
                .await
            })
            .map(move |res, actor: &mut Self, _ctx| match res {
                Ok(rows) => {
                    for row in rows {
                        let agent = AgentInfo {
                            id: row.get("id"),
                            name: row.get("name"),
                            kind: AgentKind::from_db_str(row.get::<&str, _>("kind")),
                            workspace_name: row.get("workspace_name"),
                            owner_username: row.get("owner_username"),
                            mcp_list: row.get("mcp_list"),
                        };

                        if actor.agents.contains_key(&agent.id) {
                            continue;
                        }

                        let addr = AgentActor::new(
                            agent.clone(),
                            open_aiproxy_actor.clone(),
                            mcp_manager.clone(),
                        )
                        .start();
                        actor.agents.insert(agent.id, addr);
                        actor.dag_orchestrator.do_send(RegisterAgent {
                            agent_id: agent.id,
                            name: agent.display_name(),
                            workspace_name: agent.workspace_name.clone(),
                            mcp_list: agent.mcp_list.clone(),
                        });
                        info!("♻️ 已恢复 Agent [{}] 并注册到任务编排器", agent.id);
                    }
                }
                Err(e) => {
                    error!("恢复 Agent 列表失败: {}", e);
                }
            }),
        );
    }
}

// 创建agent
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

impl Handler<CreateAgent> for AgentManageActor {
    // 1. 将返回值类型改为 ResponseActFuture
    type Result = ResponseActFuture<Self, Result<AgentInfo, AgentError>>;

    fn handle(&mut self, msg: CreateAgent, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let open_aiproxy_actor = self.open_aiproxy_actor.clone();
        let mcp_manager = self.mcp_manager.clone();
        let dag_orchestrator = self.dag_orchestrator.clone();
        let mut agent = AgentInfo::create(msg.name, msg.kind, msg.workspace_name.clone());
        agent.mcp_list = msg.mcp_list.clone();

        // 我们克隆一份用于在异步块里返回
        let agent_clone = agent.clone();

        // 2. 构造一个【纯异步 Future】，里面绝对不能出现 `self`
        let workspace_name = msg.workspace_name.clone();
        let fut = async move {
            // 1. 检查工作区是否存在
            let exists: (bool,) =
                sqlx::query_as("SELECT EXISTS(SELECT 1 FROM workspaces WHERE name = $1)")
                    .bind(&workspace_name)
                    .fetch_one(&pool)
                    .await
                    .map_err(AgentError::DatabaseError)?;
            if !exists.0 {
                return Err(AgentError::NotFound(workspace_name.clone()));
            }

            sqlx::query(
                r#"
                INSERT INTO agents (id, "name", kind, workspace_name, owner_username, mcp_list)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(&agent_clone.id)
            .bind(&agent_clone.name)
            .bind(agent_clone.kind.as_db_str())
            .bind(&workspace_name)
            .bind(&msg.user_name)
            .bind(&agent_clone.mcp_list)
            .execute(&pool)
            .await
            .map_err(|e| {
                error!("❌ 保存失败: {}", e);
                AgentError::DatabaseError(e)
            })?;

            let adir = agent_dir(&workspace_name, agent_clone.id);
            let mem_dir = agent_memory_dir(&workspace_name, agent_clone.id);
            ensure_dir(&adir).map_err(AgentError::IoError)?;
            ensure_dir(&mem_dir).map_err(AgentError::IoError)?;

            Ok((agent, open_aiproxy_actor))
        };

        // 3. 使用 wrap_future 包装异步任务，并在 map 回调中安全地修改 Actor 状态
        Box::pin(actix::fut::wrap_future(fut).map(
            move |res: Result<(AgentInfo, Addr<OpenAIProxyActor>), AgentError>,
                  actor: &mut Self,
                  _ctx| {
                match res {
                    Ok((created_agent, proxy)) => {
                        // 启动 AgentActor 并注册到管理器
                        let addr = AgentActor::new(created_agent.clone(), proxy, mcp_manager.clone()).start();
                        actor.agents.insert(created_agent.id, addr);
                        dag_orchestrator.do_send(RegisterAgent {
                            agent_id: created_agent.id,
                            name: created_agent.display_name(),
                            workspace_name: created_agent.workspace_name.clone(),
                            mcp_list: created_agent.mcp_list.clone(),
                        });
                        info!("✅ AgentActor [{}] 已注册，状态：空闲", created_agent.id);
                        Ok(created_agent)
                    }
                    Err(e) => Err(e),
                }
            },
        ))
    }
}

// --- ListAgents: 查询所有 Agent 基础信息 ---

#[derive(Message)]
#[rtype(result = "Result<Vec<AgentInfo>, AgentError>")]
pub struct ListAgents;

impl Handler<ListAgents> for AgentManageActor {
    type Result = ResponseFuture<Result<Vec<AgentInfo>, AgentError>>;

    fn handle(&mut self, _msg: ListAgents, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let rows = sqlx::query(
                r#"
                SELECT id, name, kind, workspace_name, owner_username, mcp_list
                FROM agents
                ORDER BY name ASC
                "#,
            )
            .fetch_all(&pool)
            .await
            .map_err(AgentError::DatabaseError)?;

            let agents = rows
                .into_iter()
                .map(|row| AgentInfo {
                    id: row.get("id"),
                    name: row.get("name"),
                    kind: AgentKind::from_db_str(row.get::<&str, _>("kind")),
                    workspace_name: row.get("workspace_name"),
                    owner_username: row.get("owner_username"),
                    mcp_list: row.get("mcp_list"),
                })
                .collect();

            Ok(agents)
        })
    }
}

// --- GetAgent: 查询单个 Agent 基础信息 ---

#[derive(Message)]
#[rtype(result = "Result<AgentInfo, AgentError>")]
pub struct GetAgent {
    pub agent_id: AgentId,
}

impl Handler<GetAgent> for AgentManageActor {
    type Result = ResponseFuture<Result<AgentInfo, AgentError>>;

    fn handle(&mut self, msg: GetAgent, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();

        Box::pin(async move {
            let row = sqlx::query(
                r#"
                SELECT id, name, kind, workspace_name, owner_username, mcp_list
                FROM agents
                WHERE id = $1
                "#,
            )
            .bind(msg.agent_id)
            .fetch_optional(&pool)
            .await
            .map_err(AgentError::DatabaseError)?;

            match row {
                Some(row) => Ok(AgentInfo {
                    id: row.get("id"),
                    name: row.get("name"),
                    kind: AgentKind::from_db_str(row.get::<&str, _>("kind")),
                    workspace_name: row.get("workspace_name"),
                    owner_username: row.get("owner_username"),
                    mcp_list: row.get("mcp_list"),
                }),
                None => Err(AgentError::NotFound(msg.agent_id.to_string())),
            }
        })
    }
}

// --- DeleteAgent: 删除 Agent 基础信息及管理器中的运行实例 ---

#[derive(Message)]
#[rtype(result = "Result<(), AgentError>")]
pub struct DeleteAgent {
    pub agent_id: AgentId,
}

impl Handler<DeleteAgent> for AgentManageActor {
    type Result = ResponseActFuture<Self, Result<(), AgentError>>;

    fn handle(&mut self, msg: DeleteAgent, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let agent_id = msg.agent_id;

        let fut = async move {
            let result = sqlx::query(
                r#"
                DELETE FROM agents
                WHERE id = $1
                "#,
            )
            .bind(agent_id)
            .execute(&pool)
            .await
            .map_err(AgentError::DatabaseError)?;

            if result.rows_affected() == 0 {
                return Err(AgentError::NotFound(agent_id.to_string()));
            }

            Ok(())
        };

        Box::pin(actix::fut::wrap_future(fut).map(move |res, actor: &mut Self, _ctx| {
            if res.is_ok() {
                actor.agents.remove(&agent_id);
                info!("🗑️ AgentActor [{}] 已从管理器移除", agent_id);
            }
            res
        }))
    }
}

// --- QueryAgentStatus: 查询单个 Agent 状态 ---

#[derive(Message)]
#[rtype(result = "Result<AgentStatusInfo, AgentError>")]
pub struct QueryAgentStatus {
    pub agent_id: AgentId,
}

impl Handler<QueryAgentStatus> for AgentManageActor {
    type Result = ResponseFuture<Result<AgentStatusInfo, AgentError>>;

    fn handle(&mut self, msg: QueryAgentStatus, _ctx: &mut Self::Context) -> Self::Result {
        match self.agents.get(&msg.agent_id) {
            Some(addr) => {
                let addr = addr.clone();
                Box::pin(async move {
                    addr.send(GetStatusInfo)
                        .await
                        .map_err(AgentError::MailboxError)
                })
            }
            None => Box::pin(async move {
                Err(AgentError::NotFound(msg.agent_id.to_string()))
            }),
        }
    }
}

// --- ListAgentStatuses: 查询所有 Agent 状态 ---

#[derive(Message)]
#[rtype(result = "Vec<AgentStatusInfo>")]
pub struct ListAgentStatuses;

impl Handler<ListAgentStatuses> for AgentManageActor {
    type Result = ResponseFuture<Vec<AgentStatusInfo>>;

    fn handle(&mut self, _msg: ListAgentStatuses, _ctx: &mut Self::Context) -> Self::Result {
        let agents: Vec<(AgentId, Addr<AgentActor>)> = self
            .agents
            .iter()
            .map(|(id, addr)| (*id, addr.clone()))
            .collect();

        Box::pin(async move {
            let mut result = Vec::new();
            for (_id, addr) in agents {
                if let Ok(info) = addr.send(GetStatusInfo).await {
                    result.push(info);
                }
            }
            result
        })
    }
}

// --- UpdateAgentStatus: 更新指定 Agent 状态 ---

#[derive(Message, Deserialize)]
#[rtype(result = "Result<(), AgentError>")]
pub struct UpdateAgentStatus {
    pub agent_id: AgentId,
    pub status: AgentStatus,
}

impl Handler<UpdateAgentStatus> for AgentManageActor {
    type Result = ResponseFuture<Result<(), AgentError>>;

    fn handle(&mut self, msg: UpdateAgentStatus, _ctx: &mut Self::Context) -> Self::Result {
        match self.agents.get(&msg.agent_id) {
            Some(addr) => {
                let addr = addr.clone();
                Box::pin(async move {
                    addr.send(SetStatus(msg.status))
                        .await
                        .map_err(AgentError::MailboxError)
                })
            }
            None => Box::pin(async move {
                Err(AgentError::NotFound(msg.agent_id.to_string()))
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskExecutionResult {
    pub success: bool,
    pub output: String,
}

#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "Result<AgentTaskExecutionResult, AgentError>")]
pub struct ExecuteTask {
    pub task_id: String,
    pub input: String,
    pub mcp_names: Vec<String>,
}

#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "Result<AgentTaskExecutionResult, AgentError>")]
pub struct ExecuteAgentTask {
    pub agent_id: AgentId,
    pub task_id: String,
    pub input: String,
    pub mcp_names: Vec<String>,
}

impl Handler<ExecuteAgentTask> for AgentManageActor {
    type Result = ResponseFuture<Result<AgentTaskExecutionResult, AgentError>>;

    fn handle(&mut self, msg: ExecuteAgentTask, _ctx: &mut Self::Context) -> Self::Result {
        match self.agents.get(&msg.agent_id) {
            Some(addr) => {
                let addr = addr.clone();
                let task_msg = ExecuteTask {
                    task_id: msg.task_id,
                    input: msg.input,
                    mcp_names: msg.mcp_names,
                };
                Box::pin(async move {
                    addr.send(task_msg)
                        .await
                        .map_err(AgentError::MailboxError)?
                })
            }
            None => Box::pin(async move {
                Err(AgentError::NotFound(msg.agent_id.to_string()))
            }),
        }
    }
}

// ======================== AgentActor ========================

/// 智能体 Actor
pub struct AgentActor {
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    mcp_manager: Addr<McpManagerActor>,
    id: AgentId,
    name: String,
    kind: AgentKind,
    status: AgentStatus,
    /// 使用的代理商名称（对应 OpenAIProxyActor 中已注册的 provider）
    provider: String,
    /// 使用的模型名称；空字符串表示使用该代理商的默认模型
    model: String,
    mcp_list: Vec<String>,
}

impl AgentActor {
    pub fn new(
        agent_info: AgentInfo,
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        mcp_manager: Addr<McpManagerActor>,
    ) -> Self {
        let mcp_list = agent_info.mcp_list.clone();
        AgentActor {
            open_aiproxy_actor,
            mcp_manager,
            id: agent_info.id,
            name: agent_info.name,
            kind: agent_info.kind,
            status: AgentStatus::Idle,
            provider: String::new(), // 空字符串 → 使用 OpenAIProxyActor 默认代理商
            model: String::new(),    // 空字符串 → 使用该代理商默认模型
            mcp_list,
        }
    }

    /// 指定代理商和模型
    pub fn with_provider(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = provider.into();
        self.model = model.into();
        self
    }
}

impl Actor for AgentActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("AgentActor [{}] 已启动，状态：{}", self.id, self.status.label());
    }
}

// --- GetStatusInfo: 获取 Agent 状态快照 ---

#[derive(Message)]
#[rtype(result = "AgentStatusInfo")]
pub struct GetStatusInfo;

impl Handler<GetStatusInfo> for AgentActor {
    type Result = actix::MessageResult<GetStatusInfo>;

    fn handle(&mut self, _: GetStatusInfo, _: &mut Context<Self>) -> Self::Result {
        actix::MessageResult(AgentStatusInfo {
            agent_id: self.id,
            name: self.name.clone(),
            status: self.status.clone(),
            status_label: self.status.label().to_string(),
        })
    }
}

// --- SetStatus: 更新 Agent 状态 ---

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetStatus(pub AgentStatus);

impl Handler<SetStatus> for AgentActor {
    type Result = ();

    fn handle(&mut self, msg: SetStatus, _: &mut Context<Self>) {
        info!(
            "AgentActor [{}] 状态变更：{} → {}",
            self.id,
            self.status.label(),
            msg.0.label()
        );
        self.status = msg.0;
    }
}

impl Handler<ExecuteTask> for AgentActor {
    type Result = ResponseFuture<Result<AgentTaskExecutionResult, AgentError>>;

    fn handle(&mut self, msg: ExecuteTask, _ctx: &mut Context<Self>) -> Self::Result {
        let mcp_manager = self.mcp_manager.clone();
        let task_id = msg.task_id;
        let input = msg.input;
        let mcp_names = msg.mcp_names;

        Box::pin(async move {
            let mut outputs = Vec::new();
            let mut all_success = true;

            for mcp_name in mcp_names {
                match mcp_manager
                    .send(ExecuteMcpTool {
                        name: mcp_name.clone(),
                        input: input.clone(),
                        timeout_secs: 30,
                    })
                    .await
                {
                    Ok(Ok(res)) => {
                        outputs.push(format!("{}: {}", res.name, res.output));
                        if !res.success {
                            all_success = false;
                        }
                    }
                    Ok(Err(e)) => {
                        outputs.push(format!("{}: {}", mcp_name, e));
                        all_success = false;
                    }
                    Err(e) => {
                        outputs.push(format!("{}: actor err: {}", mcp_name, e));
                        all_success = false;
                    }
                }
            }

            info!(task_id = %task_id, success = %all_success, "AgentActor 任务执行完成");

            Ok(AgentTaskExecutionResult {
                success: all_success,
                output: outputs.join("\n"),
            })
        })
    }
}

