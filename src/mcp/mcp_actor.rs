use actix::{Actor, Context, Handler, Message, ResponseActFuture, WrapFuture};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info};

use crate::chat::openai_actor::OpenAIProxyActor;
use crate::mcp::model::{McpError, McpToolDefinition};
use crate::workspace::model::AgentId;

const MCPS_DIR: &str = ".mcps";

pub struct McpAgentActor {
    open_aiproxy_actor: actix::Addr<OpenAIProxyActor>,
    mcp_list: HashMap<String, McpToolDefinition>,
    prompt: String,
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
    fn load_all_configs(mcp_dir: &PathBuf) -> Result<HashMap<String, McpToolDefinition>, McpError> {
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
                    if let Ok(config) = serde_json::from_str::<McpToolDefinition>(&content) {
                        config_list.insert(config.tool_id.clone(), config);
                    }
                }
            }
        }
        let mcp_tool_definition= McpToolDefinition::default();
        debug!("Loaded MCP configs: {:#?}", mcp_tool_definition);
        Ok(config_list)
    }

    // mcp工具创建
    pub async fn create_mcp_tool(&mut self) -> Result<(), McpError> {
        // 创建WSAM工具
        // 调用 OpenAI API 创建工具定义
        Ok(())
    }

}

impl Actor for McpAgentActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("McpManagerActor 已启动");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpExecutionResult {
    pub output: String,
}

// 处理任务
#[derive(Message)]
#[rtype(result = "Result<McpExecutionResult, McpError>")]
pub struct ExecuteMcp {
    pub agent_id: AgentId,
    pub task_id: String,
}

impl Handler<ExecuteMcp> for McpAgentActor {
    type Result = ResponseActFuture<Self, Result<McpExecutionResult, McpError>>;

    fn handle(&mut self, msg: ExecuteMcp, _ctx: &mut Self::Context) -> Self::Result {
        let _mcp_list = self.mcp_list.clone();
        let _prompt = self.prompt.clone();
        Box::pin(
            async move {
                // TODO: 分析工具调用
                // TODO: 实际执行 MCP 的逻辑

                Err(McpError::Message("ExecuteMcp not implemented".into()))
            }
            .into_actor(self),
        )
    }
}
