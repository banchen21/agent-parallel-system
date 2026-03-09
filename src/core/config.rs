use std::fs;
use std::path::Path;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub app_name: String,
    pub environment: String,
    pub app_url: String,
    pub chat_agent: ChatAgentConfig,
    pub memory_agent: MemoryAgentConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatAgentConfig {
    pub prompt_template: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryAgentConfig {
    pub prompt_template: String,
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

        let s = Config::builder()
            // 默认值
            .set_default(
                "chat_agent.prompt_template",
                r#"用户消息内容：{context:""}
请严格按照json回复。
以下是json格式:{context:""}"#,
            )?
            .set_default(
                "memory_agent.prompt_template",
                r#"用户消息内容：{context:""}
请严格按照json回复。
以下是json格式:{context:""}"#,
            )?
            // 从配置文件加载
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
            let default_content = r#"# 默认配置文件
# 应用程序基本配置
app_name = "Agent Parallel System"
environment = "development"
app_url = "http://0.0.0.0:8000"

# 聊天代理配置
[chat_agent]
prompt_template = """用户消息内容：{context:""}
请严格按照json回复。
以下是json格式:{context:""}"""
"#;

            fs::write(config_path, default_content)
                .map_err(|e| ConfigError::Message(format!("无法创建默认配置文件: {}", e)))?;

            info!("默认配置文件 {} 创建成功", config_path);
        }

        Ok(())
    }

    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}

// 全局配置实例
lazy_static::lazy_static! {
    pub static ref CONFIG: Settings = Settings::new().expect("Failed to load configuration");
}
