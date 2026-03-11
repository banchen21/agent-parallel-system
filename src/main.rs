use actix_cors::Cors;
use actix_web::cookie::time;
use actix_web::http::header;
use actix_web::{App, HttpServer, web};
use anyhow::{Context, Result};
use async_openai::config::OpenAIConfig;
use chrono::Utc;
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tracing::{error, info};

mod channel;
mod core;

// 聊天层消息处理
mod chat;

// 任务层处理
mod task_handler;

// neo4j图数据库记忆层
mod graph_memory;

use crate::api::auth::Auth;
// 引入通道层模块
use crate::channel::actor_database::DatabaseManager;
use crate::chat::actor_messages::ChannelManagerActor;
use crate::chat::chat_agent::ChatAgent;
use crate::chat::openai_actor::OpenAIProxyActor;
use crate::core::actor_system::SysMonitorActor;
use crate::core::config::CONFIG;
use crate::core::handler::get_stats_handler;
use crate::graph_memory::actor_memory::{AgentMemoryHActor, QueryMemory};
use crate::lib::ensure_database_exists;
use crate::task_handler::task_agent::TaskAgent;
use crate::utils::env_util::env_var_or_default;
use actix::Actor;
mod api;
mod lib;
// 工具
mod utils;
use chrono::Local;

#[actix_web::main]
async fn main() -> Result<()> {
    unsafe {
        std::env::set_var("TZ", "Asia/Shanghai");
    }
    println!("当前时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    dotenv().ok();

    // 从环境变量读取日志级别，默认为 info
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("正在启动 Agent Parallel System (Actix架构)...");

    // 系统监控
    let sys_monitor_actor = SysMonitorActor::new().start();

    // OpenAi 并行处理层
    let mut openai_config = OpenAIConfig::default();
    let api_base = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "null".to_string());
    openai_config = openai_config.with_api_base(api_base);
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "null".to_string());
    openai_config = openai_config.with_api_key(api_key);

    let model = env_var_or_default("OPENAI_MODEL", "gpt-3.5-turbo".to_string());
    let timeout_secs = env_var_or_default("OPENAI_TIMEOUT_SECONDS", 60u64);
    let max_tokens = env_var_or_default("OPENAI_MAX_TOKENS", 2048u32);

    let open_aiproxy_actor =
        OpenAIProxyActor::new(openai_config, model, timeout_secs, max_tokens).start();

    // postgresql
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost/agent_parallel_system".to_string()
    });
    ensure_database_exists(&database_url).await?;
    let pool = PgPoolOptions::new()
        .max_connections(50) // 增加最大连接数
        .min_connections(5) // 最小连接数
        .acquire_timeout(std::time::Duration::from_secs(3)) // 获取连接超时
        .idle_timeout(std::time::Duration::from_secs(10)) // 空闲超时
        .connect(&database_url)
        .await?;
    info!("PostgreSQL 连接成功");

    let config = CONFIG.clone();

    // neo4j 图数据库+智能记忆管理体
    let agent_memory_prompt = config.memory_agent;
    let neo4j_uri = env_var_or_default("NEO4J_URI", "127.0.0.1:7687".to_string());
    let neo4j_user = env_var_or_default("NEO4J_USERNAME", "neo4j".to_string());
    let neo4j_pass = env_var_or_default("NEO4J_PASSWORD", "neo4j".to_string());

    let agent_memory_hactor = AgentMemoryHActor::new(
        &neo4j_uri,
        &neo4j_user,
        &neo4j_pass,
        open_aiproxy_actor.clone(),
        agent_memory_prompt,
    )
    .await
    .expect("无法连接到 Neo4j");
    let agent_memory_hactor_addr = agent_memory_hactor.start();
    info!("Neo4j 连接成功");

    // Redis
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_actor = crate::api::redis_actor::RedisActor::new(&redis_url).await?;
    let redis_addr = redis_actor.start();
    let persistence = DatabaseManager::new(pool.clone());
    persistence.initialize_database().await?;
    info!("Redis 连接成功");

    info!("数据库表结构初始化完成");

    // 消息通道管理器
    let channel_manager = ChannelManagerActor::new(pool.clone()).start();

    // 用户管理器Actor
    let user_manager = crate::api::user::actor_user::UserManagerActor::new(pool.clone()).start();

    // 初始化任务Agent
    let task_agent_prompt = config.task_agent.prompt;
    let task_agent = TaskAgent::new(open_aiproxy_actor.clone(), task_agent_prompt).start();

    // 初始化聊天agent
    let chat_agent_prompt = config.chat_agent.prompt;
    let chat_agent = ChatAgent::new(
        channel_manager.clone(),
        agent_memory_hactor_addr.clone(),
        open_aiproxy_actor.clone(),
        task_agent.clone(),
        chat_agent_prompt,
    )
    .start();

    // info!("HTTP 服务器启动在 http://0.0.0.0:8000");
    // info!("API 端点:");
    // info!("  POST /auth/register - 用户注册");
    // info!("  POST /auth/login    - 用户登录");
    // info!("  POST /auth/refresh - 刷新Token");
    // info!("  POST /api/v1/message  - 通用消息接口");
    // info!("  GET /api/v1/message  - 通用消息接口(获取聊天记录)");

    // 启动 HTTP 服务器
    let server_result = HttpServer::new(move || {
        let cors = Cors::default()
            // 允许的来源 (开发环境下可以允许所有，生产环境建议指定具体域名)
            // .allow_any_origin() // 如果你想完全放开，可以使用这个
            .allow_any_origin() // 比如你的前端运行在 3000 端口
            // 允许的 HTTP 方法
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            // 允许的 Header
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::ACCEPT,
                header::CONTENT_TYPE,
            ])
            // 关键：如果你使用了自定义 Header 传递 Refresh Token，必须在这里添加
            .expose_headers(vec![header::CONTENT_DISPOSITION]) // 如果有文件下载需要暴露这个
            .allowed_header("X-Refresh-Token")
            // 允许发送 Cookie (如果需要)
            .supports_credentials()
            // 预检请求 (OPTIONS) 的缓存时间
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(agent_memory_hactor_addr.clone()))
            .app_data(web::Data::new(channel_manager.clone()))
            .app_data(web::Data::new(user_manager.clone()))
            .app_data(web::Data::new(redis_addr.clone()))
            .app_data(web::Data::new(chat_agent.clone()))
            .app_data(web::Data::new(sys_monitor_actor.clone()))
            .configure(configure_api_routes)
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await;

    match server_result {
        Ok(_) => info!("服务器正常关闭"),
        Err(e) => error!("服务器错误: {}", e),
    }

    Ok(())
}

fn configure_api_routes(cfg: &mut web::ServiceConfig) {
    // --- 1. 公开作用域：完全没有 wrap(Auth) ---
    cfg.service(
        web::scope("/auth")
            .service(api::user::handler::register) // 最终路径: /auth/register
            .service(api::user::handler::login)
            .service(api::user::handler::refresh),
    );

    // --- 2. 受保护作用域 ---
    cfg.service(
        web::scope("/api/v1")
            .wrap(Auth) // 只对 /api/v1 下的路由生效
            // 系统资源监控
            .service(get_stats_handler)
            // 聊天相关接口
            .service(chat::handler::handle_message)
            .service(chat::handler::get_message_history),
    );
}
