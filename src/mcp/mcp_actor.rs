use actix::{Actor, ActorFutureExt, Addr, Context, Handler, Message, ResponseActFuture, WrapFuture};
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::chat::openai_actor::{CallOpenAI, OpenAIProxyActor};
use crate::mcp::model::{McpError, McpToolDefinition};
use crate::mcp::mcp_util::execute_builtin_tool;
use crate::utils::json_util::clean_json_string;
use crate::utils::workspace_path::{ensure_dir, workspace_dir};
use crate::workspace::model::AgentId;

const MCPS_DIR: &str = ".mcps";

pub struct McpAgentActor {
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    pool: sqlx::PgPool,
    mcp_list: HashMap<String, McpToolDefinition>,
    prompt: String,
}

#[derive(Debug, Clone)]
struct McpTaskContext {
    id: uuid::Uuid,
    name: String,
    description: String,
    status: String,
    workspace_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolSelectionResponse {
    tool_id: Option<String>,
    #[serde(default)]
    create_new_tool: bool,
    new_tool_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolInterpretationResponse {
    summary: String,
}

#[derive(Debug, Clone)]
struct McpExecutionOutcome {
    result: McpExecutionResult,
    created_tool: Option<McpToolDefinition>,
}

impl McpAgentActor {
    /// 创建 MCP 执行 Actor，并在启动时加载本地 .mcps 工具定义。
    pub fn new(pool: sqlx::PgPool, open_aiproxy_actor: Addr<OpenAIProxyActor>, prompt: String) -> Self {
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
            pool,
            mcp_list,
            prompt,
        }
    }

    /// 从 .mcps 目录加载所有工具配置（tool_id -> 定义）。
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

    /// 根据 task_id 查询任务上下文，供工具选择/参数生成使用。
    async fn load_task_context(pool: &sqlx::PgPool, task_id: uuid::Uuid) -> Result<McpTaskContext, McpError> {
        let row = sqlx::query(
            "SELECT id, name, description, status, workspace_name FROM tasks WHERE id = $1",
        )
        .bind(task_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| McpError::Message(format!("查询任务失败: {}", e)))?;

        let row = row.ok_or_else(|| McpError::NotFound(format!("task {} not found", task_id)))?;

        Ok(McpTaskContext {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            status: row.get("status"),
            workspace_name: row.get("workspace_name"),
        })
    }

    /// 查询 agent 在数据库中声明可用的 MCP 工具列表。
    async fn load_agent_tool_ids(pool: &sqlx::PgPool, agent_id: AgentId) -> Result<Vec<String>, McpError> {
        let row = match sqlx::query("SELECT mcp_list FROM agents WHERE id = $1")
            .bind(agent_id)
            .fetch_optional(pool)
            .await
        {
            Ok(row) => row,
            Err(e) => {
                let err_text = e.to_string();
                if err_text.contains("column \"mcp_list\" does not exist") {
                    warn!(
                        "agents.mcp_list 列不存在，降级为 agent 全量工具可用模式（agent_id={}）",
                        agent_id
                    );
                    return Ok(Vec::new());
                }

                return Err(McpError::Message(format!("查询 agent MCP 列表失败: {}", err_text)));
            }
        };

        let Some(row) = row else {
            return Ok(Vec::new());
        };

        Ok(row.get::<Vec<String>, _>("mcp_list"))
    }

    /// 获取 MCP 执行默认提示词；若配置为空则回退到内置提示词。
    fn default_prompt(&self) -> &str {
        if self.prompt.trim().is_empty() {
            "你是 MCP 工具执行代理。你的职责是根据任务内容选择合适工具、必要时创建工具、生成工具参数，并把结果解释为结构化输出。所有 JSON 输出都必须可直接解析。"
        } else {
            &self.prompt
        }
    }

    /// 从全量工具中筛出该 agent 可用工具；若 agent 未配置则默认可用全部。
    fn filter_tools(
        known_tools: &HashMap<String, McpToolDefinition>,
        agent_tool_ids: &[String],
    ) -> Vec<McpToolDefinition> {
        if agent_tool_ids.is_empty() {
            return known_tools.values().cloned().collect();
        }

        agent_tool_ids
            .iter()
            .filter_map(|tool_id| known_tools.get(tool_id).cloned())
            .collect()
    }

    /// 是否启用模拟模式（启用后跳过 LLM 选择/解释，优先使用兜底逻辑）。
    fn simulation_enabled() -> bool {
        std::env::var("MCP_SIMULATE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    /// 将 MCP 执行步骤日志写入 `.workspaces/<workspace_name>/logs/mcp_flow.log`。
    fn append_workspace_log(
        workspace_name: &str,
        agent_id: AgentId,
        task_id: &str,
        stage: &str,
        detail: &str,
    ) -> Result<(), McpError> {
        let logs_dir = workspace_dir(workspace_name).join("logs");
        ensure_dir(&logs_dir)?;
        let file_path = logs_dir.join("mcp_flow.log");

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        let line = format!(
            "{} | agent={} | task={} | stage={} | {}\n",
            chrono::Utc::now().to_rfc3339(),
            agent_id,
            task_id,
            stage,
            detail
        );
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// 将任意文本标准化为可用于文件名/工具 id 的安全字符串。
    fn sanitize_tool_id(raw: &str) -> String {
        let mut sanitized = raw
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>();

        while sanitized.contains("__") {
            sanitized = sanitized.replace("__", "_");
        }

        sanitized.trim_matches('_').to_string()
    }

    /// 在无可用工具时，为当前任务构建一个基础工具定义。
    fn build_generated_tool(task: &McpTaskContext, description: Option<String>) -> McpToolDefinition {
        let suffix = Self::sanitize_tool_id(&task.name);
        let tool_id = if suffix.is_empty() {
            format!("generated_tool_{}", task.id)
        } else {
            format!("generated_{}_{}", suffix, task.id)
        };

        let mut properties = Map::new();
        properties.insert(
            "command".to_string(),
            json!({
                "type": "string",
                "description": "要执行的 shell 命令，例如: ls -la"
            }),
        );
        properties.insert(
            "cwd".to_string(),
            json!({
                "type": "string",
                "description": "命令执行目录（相对工作区），默认 ."
            }),
        );
        properties.insert(
            "timeout_ms".to_string(),
            json!({
                "type": "integer",
                "description": "命令超时毫秒，范围 500~120000"
            }),
        );

        McpToolDefinition {
            tool_id,
            description: description.unwrap_or_else(|| {
                format!("为任务 '{}' 自动生成的 MCP 工具定义", task.name)
            }),
            parameters: crate::mcp::model::McpParameters {
                r#type: "object".to_string(),
                properties,
                required: vec!["command".to_string()],
            },
            options: crate::mcp::model::McpOptions {
                timeout_ms: 30_000,
                max_retries: 1,
            },
            execution: Some(crate::mcp::model::McpExecutionConfig {
                transport: "builtin".to_string(),
                endpoint: "terminal_run".to_string(),
                method: "POST".to_string(),
                headers: std::collections::HashMap::new(),
            }),
        }
    }

    /// 将工具定义持久化到 .mcps 目录，供后续加载复用。
    fn persist_tool_definition(tool: &McpToolDefinition) -> Result<(), McpError> {
        let path = PathBuf::from(MCPS_DIR).join(format!("{}.json", tool.tool_id));
        let content = serde_json::to_string_pretty(tool)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn delete_tool_definition(tool_id: &str) -> Result<(), McpError> {
        let path = PathBuf::from(MCPS_DIR).join(format!("{}.json", tool_id));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// 调用 OpenAIProxyActor 并要求返回 JSON，统一做清洗与解析。
    async fn ask_llm_json(
        openai: Addr<OpenAIProxyActor>,
        system_prompt: String,
        user_prompt: String,
    ) -> Result<Value, McpError> {
        let response = openai
            .send(CallOpenAI {
                chat_completion_request_message: vec![
                    ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                        content: ChatCompletionRequestSystemMessageContent::Text(system_prompt),
                        name: None,
                    }),
                    ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                        name: Some("mcp_agent".to_string()),
                        content: ChatCompletionRequestUserMessageContent::Text(user_prompt),
                    }),
                ],
                tools: None,
                tool_choice: None,
                provider: None,
                model: None,
            })
            .await
            .map_err(McpError::from)?
            .map_err(|e| McpError::Message(format!("调用 OpenAI 失败: {}", e)))?;

        serde_json::from_str(clean_json_string(&response))
            .map_err(|e| McpError::Message(format!("解析 LLM JSON 失败: {}", e)))
    }

    /// 选择最合适工具；若无可用工具或模型要求创建，则自动生成工具定义。
    async fn select_or_create_tool(
        openai: Addr<OpenAIProxyActor>,
        prompt: String,
        task: &McpTaskContext,
        available_tools: &[McpToolDefinition],
    ) -> Result<(McpToolDefinition, bool), McpError> {
        if available_tools.is_empty() {
            let tool = Self::build_generated_tool(task, None);
            Self::persist_tool_definition(&tool)?;
            return Ok((tool, true));
        }

        if available_tools.len() == 1 {
            return Ok((available_tools[0].clone(), false));
        }

        if Self::simulation_enabled() {
            return Ok((available_tools[0].clone(), false));
        }

        let tools_json = serde_json::to_string_pretty(available_tools)?;
        let system_prompt = format!(
            "{}\n请根据任务内容从可用 MCP 工具列表中选择最适合的一个工具。必须只返回 JSON，例如：{{\"tool_id\":\"xxx\",\"create_new_tool\":false,\"new_tool_description\":null}}。如果现有工具都不适合，请返回 create_new_tool=true。",
            prompt
        );
        let user_prompt = format!(
            "任务标题: {}\n任务描述: {}\n任务状态: {}\n可用工具列表:\n{}",
            task.name, task.description, task.status, tools_json
        );

        let selection = Self::ask_llm_json(openai, system_prompt, user_prompt).await;

        match selection {
            Ok(value) => {
                let selection: ToolSelectionResponse = serde_json::from_value(value)
                    .map_err(|e| McpError::Message(format!("解析工具选择结果失败: {}", e)))?;

                if selection.create_new_tool {
                    let tool = Self::build_generated_tool(task, selection.new_tool_description);
                    Self::persist_tool_definition(&tool)?;
                    return Ok((tool, true));
                }

                if let Some(tool_id) = selection.tool_id {
                    if let Some(tool) = available_tools.iter().find(|tool| tool.tool_id == tool_id) {
                        return Ok((tool.clone(), false));
                    }
                }

                Ok((available_tools[0].clone(), false))
            }
            Err(_) => Ok((available_tools[0].clone(), false)),
        }
    }

    /// 在模型生成参数失败时，按任务内容构建一个兜底参数对象。
    fn fallback_arguments(task: &McpTaskContext, tool: &McpToolDefinition) -> Value {
        let mut args = Map::new();

        for required in &tool.parameters.required {
            let value = if required.contains("command") {
                let escaped = task.description.replace('"', "\\\"");
                Value::String(format!("echo \"{}\"", escaped))
            } else if required.contains("name") {
                Value::String(task.name.clone())
            } else if required.contains("workspace") {
                Value::String(task.workspace_name.clone().unwrap_or_default())
            } else {
                Value::String(task.description.clone())
            };
            args.insert(required.clone(), value);
        }

        if args.is_empty() {
            args.insert(
                "input".to_string(),
                Value::String(format!("{}\n{}", task.name, task.description)),
            );
        }

        Value::Object(args)
    }

    /// 基于任务与工具 schema 生成工具调用参数，失败时回退到本地兜底参数。
    async fn generate_tool_arguments(
        openai: Addr<OpenAIProxyActor>,
        prompt: String,
        task: &McpTaskContext,
        tool: &McpToolDefinition,
    ) -> Result<Value, McpError> {
        if Self::simulation_enabled() {
            return Ok(Self::fallback_arguments(task, tool));
        }

        if tool.parameters.properties.is_empty() {
            return Ok(Self::fallback_arguments(task, tool));
        }

        let schema_json = serde_json::to_string_pretty(&tool.parameters)?;
        let system_prompt = format!(
            "{}\n请为指定 MCP 工具生成参数。必须返回单个 JSON 对象，字段必须符合 schema。不要返回解释。",
            prompt
        );
        let user_prompt = format!(
            "任务标题: {}\n任务描述: {}\n工具ID: {}\n工具描述: {}\n参数Schema:\n{}",
            task.name, task.description, tool.tool_id, tool.description, schema_json
        );

        match Self::ask_llm_json(openai, system_prompt, user_prompt).await {
            Ok(Value::Object(map)) => Ok(Value::Object(map)),
            Ok(_) => Ok(Self::fallback_arguments(task, tool)),
            Err(_) => Ok(Self::fallback_arguments(task, tool)),
        }
    }

    /// 生成“调用计划”形式的原始输出（当前用于无后端执行时的可观测结果）。
    fn build_execution_plan_output(task: &McpTaskContext, tool: &McpToolDefinition, arguments: &Value) -> Result<String, McpError> {
        Ok(serde_json::to_string_pretty(&json!({
            "task_id": task.id,
            "task_name": task.name,
            "tool_id": tool.tool_id,
            "arguments": arguments,
            "note": "MCP 调用计划已生成，但当前仓库尚未实现真实的外部 MCP 执行后端。"
        }))?)
    }

    /// 按工具配置执行真实 MCP 调用，并根据 max_retries 做重试。
    async fn execute_tool_with_retry(tool: &McpToolDefinition, arguments: &Value) -> Result<Value, McpError> {
        let attempts = tool.options.max_retries.max(1);
        let mut last_err: Option<McpError> = None;

        for _ in 0..attempts {
            match Self::execute_tool_once(tool, arguments).await {
                Ok(output) => return Ok(output),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| McpError::Message("工具执行失败（未知错误）".to_string())))
    }

    /// 执行一次 MCP 调用：优先走 execution 配置，否则尝试内置工具实现。
    async fn execute_tool_once(tool: &McpToolDefinition, arguments: &Value) -> Result<Value, McpError> {
        if let Some(exec) = &tool.execution {
            if exec.transport.eq_ignore_ascii_case("http") {
                return Self::execute_http_tool(tool, arguments).await;
            }
            if exec.transport.eq_ignore_ascii_case("builtin") {
                return execute_builtin_tool(&exec.endpoint, arguments).await;
            }
            return Err(McpError::Message(format!(
                "不支持的 transport: {}",
                exec.transport
            )));
        }

        // 内置工具后端：在未配置 execution 时，给常用工具提供真实执行能力。
        execute_builtin_tool(&tool.tool_id, arguments).await
    }

    /// HTTP 执行器：GET 走 query，其他方法走 JSON body。
    async fn execute_http_tool(tool: &McpToolDefinition, arguments: &Value) -> Result<Value, McpError> {
        let exec = tool
            .execution
            .as_ref()
            .ok_or_else(|| McpError::Message("missing execution config".to_string()))?;
        let method = if exec.method.trim().is_empty() {
            "POST".to_string()
        } else {
            exec.method.to_uppercase()
        };
        let client = reqwest::Client::new();
        let timeout_ms = if tool.options.timeout_ms == 0 {
            30_000
        } else {
            tool.options.timeout_ms
        };

        let fut = async {
            let mut req = match method.as_str() {
                "GET" => {
                    let mut builder = client.get(&exec.endpoint);
                    if let Value::Object(map) = arguments {
                        let mut params: Vec<(String, String)> = Vec::new();
                        for (k, v) in map {
                            let s = if let Some(ss) = v.as_str() {
                                ss.to_string()
                            } else {
                                v.to_string()
                            };
                            params.push((k.clone(), s));
                        }
                        builder = builder.query(&params);
                    }
                    builder
                }
                "PUT" => client.put(&exec.endpoint).json(arguments),
                "PATCH" => client.patch(&exec.endpoint).json(arguments),
                "DELETE" => client.delete(&exec.endpoint).json(arguments),
                _ => client.post(&exec.endpoint).json(arguments),
            };

            for (k, v) in &exec.headers {
                req = req.header(k, v);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| McpError::Message(format!("HTTP 调用失败: {}", e)))?;
            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| McpError::Message(format!("读取 HTTP 响应失败: {}", e)))?;

            if !status.is_success() {
                return Err(McpError::Message(format!(
                    "HTTP 状态码异常: {}，响应: {}",
                    status, text
                )));
            }

            match serde_json::from_str::<Value>(&text) {
                Ok(v) => Ok(v),
                Err(_) => Ok(json!({ "text": text })),
            }
        };

        timeout(Duration::from_millis(timeout_ms), fut)
            .await
            .map_err(|_| McpError::Message(format!("HTTP 工具调用超时: {}ms", timeout_ms)))?
    }

    /// 对工具原始输出做摘要解释，返回上层可直接消费的文本结论。
    async fn interpret_tool_result(
        openai: Addr<OpenAIProxyActor>,
        prompt: String,
        task: &McpTaskContext,
        tool: &McpToolDefinition,
        raw_output: &str,
    ) -> Result<String, McpError> {
        if Self::simulation_enabled() {
            return Ok(format!(
                "[SIMULATE] 工具 {} 执行流程完成，已生成结构化输出。",
                tool.tool_id
            ));
        }

        let system_prompt = format!(
            "{}\n请对 MCP 工具输出做简洁解释。必须返回 JSON，例如：{{\"summary\":\"...\"}}。",
            prompt
        );
        let user_prompt = format!(
            "任务标题: {}\n任务描述: {}\n工具ID: {}\n工具输出:\n{}",
            task.name, task.description, tool.tool_id, raw_output
        );

        let value = Self::ask_llm_json(openai, system_prompt, user_prompt).await?;
        let interpretation: ToolInterpretationResponse = serde_json::from_value(value)
            .map_err(|e| McpError::Message(format!("解析结果解释失败: {}", e)))?;

        Ok(interpretation.summary)
    }

    /// 预留：创建 MCP 工具定义（后续可接真实工具注册流程）。
    pub async fn create_mcp_tool(&mut self) -> Result<(), McpError> {
        // 创建WSAM工具
        // 调用 OpenAI API 创建工具定义
        Ok(())
    }

}

impl Actor for McpAgentActor {
    type Context = Context<Self>;

    /// Actor 启动回调：记录运行日志。
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("McpManagerActor 已启动");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpExecutionResult {
    pub task_id: String,
    pub success: bool,
    pub executed: bool,
    pub should_retry: bool,
    pub selected_tool_id: Option<String>,
    pub tool_created: bool,
    pub arguments: Value,
    pub raw_output: String,
    pub interpreted_output: String,
    pub failure_reason: Option<String>,
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

    /// MCP 执行主流程：查询上下文 -> 选/建工具 -> 生成参数 -> 形成输出 -> 解释结果。
    fn handle(&mut self, msg: ExecuteMcp, _ctx: &mut Self::Context) -> Self::Result {
        let pool = self.pool.clone();
        let open_aiproxy_actor = self.open_aiproxy_actor.clone();
        let known_tools = self.mcp_list.clone();
        let prompt = self.default_prompt().to_string();
        let simulate = Self::simulation_enabled();
        Box::pin(
            async move {
                let task_uuid = uuid::Uuid::parse_str(&msg.task_id)
                    .map_err(|e| McpError::Message(format!("task_id 不是合法 UUID: {}", e)))?;
                let task = Self::load_task_context(&pool, task_uuid).await?;
                let workspace_name = task
                    .workspace_name
                    .clone()
                    .unwrap_or_else(|| "default_workspace".to_string());
                let _ = Self::append_workspace_log(
                    &workspace_name,
                    msg.agent_id,
                    &msg.task_id,
                    "start",
                    &format!("MCP execution started (simulate={})", simulate),
                );

                let agent_tool_ids = Self::load_agent_tool_ids(&pool, msg.agent_id).await?;
                let available_tools = Self::filter_tools(&known_tools, &agent_tool_ids);
                let _ = Self::append_workspace_log(
                    &workspace_name,
                    msg.agent_id,
                    &msg.task_id,
                    "tool_inventory",
                    &format!("agent tools in db: {}, usable tools: {}", agent_tool_ids.len(), available_tools.len()),
                );

                let (selected_tool, tool_created) = Self::select_or_create_tool(
                    open_aiproxy_actor.clone(),
                    prompt.clone(),
                    &task,
                    &available_tools,
                )
                .await?;
                let _ = Self::append_workspace_log(
                    &workspace_name,
                    msg.agent_id,
                    &msg.task_id,
                    "tool_selected",
                    &format!("tool_id={}, tool_created={}", selected_tool.tool_id, tool_created),
                );

                let arguments = Self::generate_tool_arguments(
                    open_aiproxy_actor.clone(),
                    prompt.clone(),
                    &task,
                    &selected_tool,
                )
                .await?;
                let _ = Self::append_workspace_log(
                    &workspace_name,
                    msg.agent_id,
                    &msg.task_id,
                    "arguments_generated",
                    &format!("arguments={}", arguments),
                );

                let execution = Self::execute_tool_with_retry(&selected_tool, &arguments).await;
                let (success, executed, should_retry, raw_output, failure_reason) = match execution {
                    Ok(v) => (
                        true,
                        true,
                        false,
                        serde_json::to_string_pretty(&v)
                            .unwrap_or_else(|_| v.to_string()),
                        None,
                    ),
                    Err(e) => (
                        false,
                        false,
                        selected_tool.options.max_retries > 0,
                        Self::build_execution_plan_output(&task, &selected_tool, &arguments)
                            .unwrap_or_else(|_| "{}".to_string()),
                        Some(format!("MCP 执行失败: {}", e)),
                    ),
                };
                let _ = Self::append_workspace_log(
                    &workspace_name,
                    msg.agent_id,
                    &msg.task_id,
                    "tool_executed",
                    &format!(
                        "success={}, executed={}, should_retry={}, failure_reason={}",
                        success,
                        executed,
                        should_retry,
                        failure_reason.clone().unwrap_or_default()
                    ),
                );

                let interpreted_output = Self::interpret_tool_result(
                    open_aiproxy_actor,
                    prompt,
                    &task,
                    &selected_tool,
                    &raw_output,
                )
                .await
                .unwrap_or_else(|_| {
                    if success {
                        "工具执行完成，但结果解释阶段失败，已返回原始输出".to_string()
                    } else {
                        "工具调用计划已生成，但执行失败，请查看 failure_reason".to_string()
                    }
                });
                let _ = Self::append_workspace_log(
                    &workspace_name,
                    msg.agent_id,
                    &msg.task_id,
                    "result_interpreted",
                    &interpreted_output,
                );

                Ok(McpExecutionOutcome {
                    result: McpExecutionResult {
                        task_id: task.id.to_string(),
                        success,
                        executed,
                        should_retry,
                        selected_tool_id: Some(selected_tool.tool_id.clone()),
                        tool_created,
                        arguments,
                        raw_output,
                        interpreted_output,
                        failure_reason,
                    },
                    created_tool: if tool_created {
                        Some(selected_tool)
                    } else {
                        None
                    },
                })
            }
            .into_actor(self)
            .map(|res: Result<McpExecutionOutcome, McpError>, actor, _ctx| match res {
                Ok(outcome) => {
                    if let Some(tool) = outcome.created_tool.clone() {
                        actor.mcp_list.insert(tool.tool_id.clone(), tool);
                    }
                    Ok(outcome.result)
                }
                Err(e) => Err(e),
            }),
        )
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<McpToolDefinition>, McpError>")]
pub struct ListMcpTools;

impl Handler<ListMcpTools> for McpAgentActor {
    type Result = Result<Vec<McpToolDefinition>, McpError>;

    fn handle(&mut self, _msg: ListMcpTools, _ctx: &mut Self::Context) -> Self::Result {
        let mut tools: Vec<McpToolDefinition> = self.mcp_list.values().cloned().collect();
        tools.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        Ok(tools)
    }
}

#[derive(Message)]
#[rtype(result = "Result<McpToolDefinition, McpError>")]
pub struct UpsertMcpTool {
    pub tool: McpToolDefinition,
}

impl Handler<UpsertMcpTool> for McpAgentActor {
    type Result = Result<McpToolDefinition, McpError>;

    fn handle(&mut self, msg: UpsertMcpTool, _ctx: &mut Self::Context) -> Self::Result {
        let mut tool = msg.tool;
        let sanitized_tool_id = Self::sanitize_tool_id(&tool.tool_id);
        if sanitized_tool_id.is_empty() {
            return Err(McpError::Message("tool_id 不能为空".to_string()));
        }

        tool.tool_id = sanitized_tool_id;

        if tool.parameters.r#type.trim().is_empty() {
            tool.parameters.r#type = "object".to_string();
        }

        if tool.options.timeout_ms == 0 {
            tool.options.timeout_ms = 30_000;
        }

        if tool.options.max_retries == 0 {
            tool.options.max_retries = 1;
        }

        if let Some(exec) = tool.execution.as_mut() {
            if exec.transport.trim().is_empty() {
                exec.transport = "http".to_string();
            }
            if exec.method.trim().is_empty() {
                exec.method = "POST".to_string();
            }
        }

        Self::persist_tool_definition(&tool)?;
        self.mcp_list.insert(tool.tool_id.clone(), tool.clone());
        Ok(tool)
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), McpError>")]
pub struct DeleteMcpTool {
    pub tool_id: String,
}

impl Handler<DeleteMcpTool> for McpAgentActor {
    type Result = Result<(), McpError>;

    fn handle(&mut self, msg: DeleteMcpTool, _ctx: &mut Self::Context) -> Self::Result {
        let tool_id = Self::sanitize_tool_id(&msg.tool_id);
        if tool_id.is_empty() {
            return Err(McpError::Message("tool_id 不能为空".to_string()));
        }

        Self::delete_tool_definition(&tool_id)?;
        self.mcp_list.remove(&tool_id);
        Ok(())
    }
}
