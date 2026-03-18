use std::fs;
use std::path::Path;

use config::{Config, ConfigError, Environment, File};
use rand::{Rng, distributions::Alphanumeric};
use regex::Regex;
use serde::Deserialize;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub app_name: String,
    pub environment: String,
    pub app_url: String,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
    pub agents: AgentsConfig,
    pub mcp_agent: McpAgentConfig,
    pub features: FeaturesConfig,
    pub chat_agent: ChatAgentConfig,
    pub memory_agent: MemoryAgentConfig,
    pub task_agent: TaskAgentConfig,
    #[serde(default)]
    pub task_review: TaskReviewConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub providers: Vec<ProviderItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub super_secret_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpAgentConfig {
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LimitsConfig {
    pub chat_history_limit: i64,
    pub api_history_limit: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentsConfig {
    pub running_loop_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeaturesConfig {
    pub enable_memory_query: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatAgentConfig {
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryAgentConfig {
    pub prompt_query: String,
    pub prompt_summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskAgentConfig {
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskReviewConfig {
    pub submitted_recover_scan_interval_secs: u64,
    pub first_retry_delay_secs: u64,
}

impl Default for TaskReviewConfig {
    fn default() -> Self {
        Self {
            submitted_recover_scan_interval_secs: 30,
            first_retry_delay_secs: 60,
        }
    }
}

/// 单个代理商的静态配置（对应 TOML 中的 [[providers]]）
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderItem {
    /// 代理商唯一名称，例如 "deepseek"、"openai"、"ollama"
    pub name: String,
    /// OpenAI 兼容接口的 base URL
    pub base_url: String,
    /// 该代理商的默认模型
    pub default_model: String,
    /// API Key（可留空，优先从环境变量 PROVIDER_{NAME大写}_API_KEY 读取）
    #[serde(default)]
    pub api_key: String,
}

/// LLM 全局参数
#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    /// 默认代理商名称；留空则使用 [[providers]] 第一项
    #[serde(default)]
    pub default_provider: String,
    pub timeout_secs: u64,
    pub max_tokens: u32,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let default_config_path = "config/default.toml";

        info!(
            "Loading configuration for environment: {}",
            default_config_path
        );

        // 检查并创建默认配置文件（如果不存在）
        Self::ensure_default_config_exists(default_config_path)?;
        Self::refresh_runtime_secret(default_config_path)?;

        let s = Config::builder()
            // 默认值
            .set_default("security.super_secret_key", "")?
            .set_default("limits.chat_history_limit", 10)?
            .set_default("limits.api_history_limit", 20)?
            .set_default("agents.running_loop_interval_secs", 3i64)?
            .set_default("mcp_agent.prompt", r#""#)?
            .set_default("chat_agent.prompt", r#""#)?
            .set_default("memory_agent.prompt_query", r#""#)?
            .set_default("memory_agent.prompt_summary", r#""#)?
            // 从配置文件加载
            .set_default("task_agent.prompt", r#""#)?
            .set_default("task_review.submitted_recover_scan_interval_secs", 30i64)?
            .set_default("task_review.first_retry_delay_secs", 60i64)?
            .set_default("llm.default_provider", "")?
            .set_default("llm.timeout_secs", 60i64)?
            .set_default("llm.max_tokens", 2048i64)?
            .add_source(File::with_name(default_config_path).required(true))
            // 从.env文件加载
            .add_source(
                Environment::with_prefix("APP")
                    .separator("_")
                    .ignore_empty(true),
            )
            .build()?;

        s.try_deserialize()
    }

    /// 确保默认配置文件存在，如果不存在则自动创建
    fn ensure_default_config_exists(config_path: &str) -> Result<(), ConfigError> {
        if !Path::new(config_path).exists() {
            warn!("配置文件 {} 不存在，正在自动创建...", config_path);

            // 确保config目录存在
            if let Some(parent) = Path::new(config_path).parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| ConfigError::Message(format!("无法创建配置目录: {}", e)))?;
            }

            // 创建默认配置内容
            let default_content = r#"
            # 默认配置文件
# 应用程序基本配置
app_name = "Agent Parallel System"
environment = "development"
app_url = "http://0.0.0.0:8000"

# 聊天代理配置
[chat_agent]
prompt_template = """你是一个高精度的意图识别并能处理任务的专家。请分析用户的消息内容，判断其是否属于一个“任务(Task)”。

【定义标准】
- 任务 (Task): 包含明确的指令、需要执行的操作、请求编写代码、分析数据、翻译、创作长文本或复杂的逻辑推理、具体执行任务。
- 非任务 (Chat): 简单的问候（你好）、无意义的闲聊、表达情绪（哈哈）、简单的常识问答或无需特殊处理的陈述。
- 非任务回复 (Chat):  根据人格设定以及记忆内容，正常回复用户消息，无需执行任何任务。

【长期记忆】
{memory_content}

【短期记忆】
{momory_content_short}

【用户内容】
{user_input}


【约束条件】
1. 必须严格按照下方的 JSON 格式回复。
2. 不得包含任何解释、前导词或后缀。
3. 确保输出可以被程序直接解析。

【输出格式】
{
    "is_task": boolean,
    "confidence": float,
    "content": "如{is_task}=true则表示正在处理同时根据【回复句式方面】进行回复,",
    "reason": "总任务标题",
    "tasks": [
        {
            "task_id": "唯一标识符",
            "task": "任务1的标题",
            "task_description": "任务1的详细描述",
        },{
            "task_id": "唯一标识符",
            "task": "任务2的标题",
            "task_description": "任务2的详细描述",
        }
    ]否则为null
}
"""

# 智能记忆代理配置
[memory_agent]
prompt_template = """你的任务是从对话中提取【持久性事实】，并以结构化形式输出。

**【节点与关系】**
- **Assistant节点**：代表你，使用名称例如 '齐悦'或者'banchen'等。
- **实体节点**：代表其他实体'。
- **关系**：用以连接Assistant节点与实体节点。

**【关系类型】**：如 `喜欢`, `讨厌`, `掌握`, `职业是`等。

**【提取规则】**
1. 仅提取长期事实（兴趣、身份、技能、项目、性格等）。
2. 忽略瞬时信息（打招呼、情绪、闲聊等无关长期事实消息）。
3. 主体必须是 '{ai_name}' 或 '{user_name}等具体的实体对象'。
4. 无事实时返回空列表。

**【本地记忆】**
{knowledge_summary}

**【{user_name}的内容】**
{user_content}

【输出格式】
{
    "graph":[
    {{ "action": "UPSERT", "subject": "...", "relation": "...", "object": "..." }},
    {{ "action": "DELETE", "subject": "...", "relation": "...", "object": "..." }}
  ]
}

"""
"#;

            fs::write(config_path, default_content)
                .map_err(|e| ConfigError::Message(format!("无法创建默认配置文件: {}", e)))?;

            info!("默认配置文件 {} 创建成功", config_path);
        }

        Ok(())
    }

    fn refresh_runtime_secret(config_path: &str) -> Result<(), ConfigError> {
        let content = fs::read_to_string(config_path)
            .map_err(|e| ConfigError::Message(format!("无法读取配置文件: {}", e)))?;
        let new_secret = Self::generate_runtime_secret();
        let secret_line = format!("super_secret_key = \"{}\"", new_secret);

        let updated = if content.contains("[security]") {
            let re = Regex::new(r#"(?m)^super_secret_key\s*=\s*\".*\"\s*$"#)
                .map_err(|e| ConfigError::Message(format!("配置正则错误: {}", e)))?;
            if re.is_match(&content) {
                re.replace(&content, secret_line.as_str()).to_string()
            } else {
                content.replacen("[security]", &format!("[security]\n{}", secret_line), 1)
            }
        } else {
            format!("{}\n\n[security]\n{}\n", content.trim_end(), secret_line)
        };

        fs::write(config_path, updated)
            .map_err(|e| ConfigError::Message(format!("无法写入运行时密钥: {}", e)))?;

        info!("已刷新并写入运行时安全密钥到 {}", config_path);
        Ok(())
    }

    fn generate_runtime_secret() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(48)
            .map(char::from)
            .collect()
    }
}

// 全局配置实例
lazy_static::lazy_static! {
    pub static ref CONFIG: Settings = Settings::new().expect("Failed to load configuration");
}
