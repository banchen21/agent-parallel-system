use actix::{Actor, ActorFutureExt, Context, Handler, Message, ResponseActFuture, WrapFuture};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::{error, info};

use crate::chat::openai_actor::OpenAIProxyActor;
use crate::mcp;
use crate::mcp::model::{McpConfig, McpError};

const MCPS_DIR: &str = ".mcps";

pub struct McpAgentActor {
    open_aiproxy_actor: actix::Addr<OpenAIProxyActor>,
    mcp_list: HashMap<String, McpConfig>,
    prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpExecutionResult {
    pub output: String,
}

impl McpAgentActor {
    pub fn new(open_aiproxy_actor: actix::Addr<OpenAIProxyActor>, prompt: String) -> Self {
        let mcp_dir = PathBuf::from(MCPS_DIR);
        // 确保 .mcps 目录存在
        if let Err(e) = fs::create_dir_all(&mcp_dir) {
            error!("创建 .mcps 目录失败: {}", e);
        } else {
            info!("已确保 .mcps 目录存在");
        }
        let mcp_list = match Self::load_all_configs(&mcp_dir) {
            Ok(list) => list,
            Err(e) => {
                error!("加载 MCP 配置失败: {}", e);
                HashMap::new()
            }
        };

        Self {
            open_aiproxy_actor,
            mcp_list,
            prompt,
        }
    }

    /// 从 .mcps 目录加载所有配置
    fn load_all_configs(mcp_dir: &PathBuf) -> Result<HashMap<String, McpConfig>, McpError> {
        let mut config_list = HashMap::new();

        if !mcp_dir.exists() {
            return Ok(config_list);
        }

        let entries = fs::read_dir(mcp_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && path.file_name().and_then(|s| s.to_str()) != Some("README.md")
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = serde_json::from_str::<McpConfig>(&content) {
                        config_list.insert(config.name.clone(), config);
                    }
                }
            }
        }

        Ok(config_list)
    }
}

impl Actor for McpAgentActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("McpManagerActor 已启动");
    }
}

// 处理任务
#[derive(Message)]
#[rtype(result = "Result<McpExecutionResult, McpError>")]
pub struct ExecuteMcp {
    agent_id: String,
    task_id: String,
}

impl Handler<ExecuteMcp> for McpAgentActor {
    type Result = ResponseActFuture<Self, Result<McpExecutionResult, McpError>>;

    fn handle(&mut self, msg: ExecuteMcp, _ctx: &mut Self::Context) -> Self::Result {
        let _mcp_list = self.mcp_list.clone();
        let _prompt = self.prompt.clone();
        // 占位实现：目前尚未实现具体执行逻辑，返回明确的错误以便编译通过并可逐步完善
        Box::pin(
            async move { Err(McpError::Message("ExecuteMcp not implemented".into())) }
                .into_actor(self),
        )
    }
}
