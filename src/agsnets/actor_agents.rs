use actix::{
    Actor, ActorFutureExt, Addr, Context, Handler, Message, ResponseActFuture, ResponseFuture,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info};

use crate::{
    agsnets::model::AgentError,
    chat::openai_actor::OpenAIProxyActor,
    utils::workspace_path::{agent_dir, agent_memory_dir, ensure_dir},
    workspace::model::{AgentId, AgentInfo, AgentKind},
};

pub struct AgentManageActor {
    agents: HashMap<AgentId, Addr<AgentActor>>,
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    pool: sqlx::PgPool,
}

impl AgentManageActor {
    pub fn new(pool: sqlx::PgPool, open_aiproxy_actor: Addr<OpenAIProxyActor>) -> Self {
        Self {
            agents: HashMap::new(),
            open_aiproxy_actor,
            pool,
        }
    }
}

impl Actor for AgentManageActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("AgentActor 已启动");
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
}

impl Handler<CreateAgent> for AgentManageActor {
    // 1. 将返回值类型改为 ResponseActFuture
    type Result = ResponseActFuture<Self, Result<AgentInfo, AgentError>>;

    fn handle(&mut self, msg: CreateAgent, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let agent = AgentInfo::create(msg.name, msg.kind, msg.workspace_name.clone());

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
                INSERT INTO agents (id, "name", kind, workspace_name,owner_username)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(&agent_clone.id)
            .bind(&agent_clone.name)
            .bind(agent_clone.kind.as_db_str())
            .bind(&workspace_name)
            .bind(&msg.user_name)
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

            Ok(agent)
        };

        // 3. 使用 wrap_future 包装异步任务，并在 map 回调中安全地修改 Actor 状态
        Box::pin(actix::fut::wrap_future(fut).map(
            move |res: Result<AgentInfo, AgentError>, actor: &mut Self, _ctx| match res {
                Ok(created_agent) => {
                    Ok(created_agent)
                }
                Err(e) => Err(e),
            },
        ))
    }
}

// 智能体状态
pub enum AgentStatus {
    Created,
    Running,
    Stopped,
}

/// 智能体
pub struct AgentActor {
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    id: AgentId,
    name: String,
    kind: AgentKind,
    status: AgentStatus,
    model: String, //代理商模型
    mcp_list: Vec<String>,
}

impl AgentActor {
    pub fn new(agent_info: AgentInfo, open_aiproxy_actor: Addr<OpenAIProxyActor>) -> Self {
        let agent = AgentActor {
            open_aiproxy_actor,
            id: agent_info.id,
            name: agent_info.name,
            kind: agent_info.kind,
            status: AgentStatus::Created,
            model: String::from("gpt-3.5-turbo"),
            mcp_list: Vec::new(),
        };
        agent
    }
}

impl Actor for AgentActor {
    type Context = Context<Self>;
}
