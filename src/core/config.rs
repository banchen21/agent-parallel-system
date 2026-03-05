use std::env;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: u64,
    pub idle_timeout: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    pub connection_timeout: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub algorithm: String,
    pub access_token_expire_minutes: i64,
    pub refresh_token_expire_days: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub bcrypt_cost: u32,
    pub rate_limit: u32,
    pub rate_limit_window: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub api_prefix: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub storage_type: String,
    pub local_path: String,
    pub max_file_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub app_name: String,
    pub environment: String,
    pub debug: bool,
    pub app_url: String,
    
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub security: SecurityConfig,
    pub openai: OpenAIConfig,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub storage: StorageConfig,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let environment = env::var("APP_ENV").unwrap_or_else(|_| "development".into());
        let config_file = format!("config/{}.toml", environment);
        
        info!("Loading configuration for environment: {}", environment);
        
        let s = Config::builder()
            // 默认值
            .set_default("app_name", "Agent Parallel System")?
            .set_default("environment", environment.clone())?
            .set_default("debug", false)?
            .set_default("app_url", "http://localhost:8000")?
            
            // 数据库默认值
            .set_default("database.max_connections", 20)?
            .set_default("database.min_connections", 5)?
            .set_default("database.connect_timeout", 10)?
            .set_default("database.idle_timeout", 300)?
            
            // Redis默认值
            .set_default("redis.pool_size", 10)?
            .set_default("redis.connection_timeout", 5)?
            
            // JWT默认值
            .set_default("jwt.algorithm", "HS256")?
            .set_default("jwt.access_token_expire_minutes", 30)?
            .set_default("jwt.refresh_token_expire_days", 7)?
            
            // 安全默认值
            .set_default("security.bcrypt_cost", 12)?
            .set_default("security.rate_limit", 100)?
            .set_default("security.rate_limit_window", 60)?
            
            // OpenAI默认值
            .set_default("openai.base_url", "https://api.openai.com/v1")?
            .set_default("openai.model", "gpt-4")?
            .set_default("openai.max_tokens", 4096)?
            .set_default("openai.temperature", 0.7)?
            
            // 服务器默认值
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8000)?
            .set_default("server.workers", 4)?
            .set_default("server.api_prefix", "/api/v1")?
            
            // 日志默认值
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            
            // 存储默认值
            .set_default("storage.storage_type", "local")?
            .set_default("storage.local_path", "./storage")?
            .set_default("storage.max_file_size", 10485760)? // 10MB
            
            // 从配置文件加载
            .add_source(File::with_name("config/default.toml").required(false))
            .add_source(File::with_name(&config_file).required(false))
            
            // 从环境变量加载（带前缀APP_）
            .add_source(Environment::with_prefix("APP").separator("_"))
            
            // 从.env文件加载
            .add_source(Environment::with_prefix("APP").separator("_").ignore_empty(true))
            
            .build()?;
        
        s.try_deserialize()
    }
    
    pub fn database_url(&self) -> &str {
        &self.database.url
    }
    
    pub fn redis_url(&self) -> &str {
        &self.redis.url
    }
    
    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
    
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
    
    pub fn api_url(&self) -> String {
        format!("{}:{}{}", self.app_url, self.server.port, self.server.api_prefix)
    }
}

// 全局配置实例
lazy_static::lazy_static! {
    pub static ref CONFIG: Settings = Settings::new().expect("Failed to load configuration");
}
