use actix_cors::Cors;
use actix_web::http::header;
use actix_web::{App, HttpResponse, HttpServer, web};
use anyhow::{Context, Result};
use async_openai::config::OpenAIConfig;
use dotenv::dotenv;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::env;
use std::ops::Add;
use tracing::{error, info};

mod channel;
mod core;

// 聊天层消息处理
mod chat;

// 任务层处理
mod task_handler;

use crate::api::auth::{Auth, AuthMiddleware};
// 引入通道层模块
use crate::channel::actor_database::DatabaseManager;
use crate::channel::actor_manager::ChannelManagerActor;
use crate::chat::chat_agent::ChatAgent;
use crate::core::config::CONFIG;
use actix::Actor;
mod api;
/// 确保数据库存在，如果不存在则创建
async fn ensure_database_exists(database_url: &str) -> Result<()> {
    // 解析数据库URL获取数据库名称
    let db_name = extract_database_name(database_url).context("无法从数据库URL中提取数据库名称")?;

    // 尝试连接到目标数据库
    match PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
    {
        Ok(_) => {
            info!("数据库 '{}' 已存在", db_name);
            return Ok(());
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains(&format!("database \"{}\" does not exist", db_name)) {
                info!("数据库 '{}' 不存在，正在创建...", db_name);

                // 从原始URL构建postgres数据库URL
                let postgres_url =
                    build_postgres_url(database_url).context("无法构建postgres数据库URL")?;

                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&postgres_url)
                    .await
                    .context("无法连接到postgres数据库")?;

                // 创建数据库
                sqlx::query(&format!("CREATE DATABASE {}", db_name))
                    .execute(&pool)
                    .await
                    .context(format!("无法创建数据库 '{}'", db_name))?;

                info!("数据库 '{}' 创建成功", db_name);
            } else {
                return Err(e).context("数据库连接失败");
            }
        }
    }

    Ok(())
}

/// 从数据库URL中提取数据库名称
fn extract_database_name(database_url: &str) -> Result<String> {
    let url_parts: Vec<&str> = database_url.split('/').collect();
    if url_parts.len() >= 4 {
        Ok(url_parts[3].to_string())
    } else {
        anyhow::bail!("无效的数据库URL格式: {}", database_url)
    }
}

/// 构建postgres数据库URL（连接到默认postgres数据库）
fn build_postgres_url(database_url: &str) -> Result<String> {
    let url_parts: Vec<&str> = database_url.split('/').collect();
    if url_parts.len() >= 3 {
        // 替换数据库名称为postgres
        let mut new_parts = url_parts.clone();
        if new_parts.len() >= 4 {
            new_parts[3] = "postgres";
        }
        Ok(new_parts.join("/"))
    } else {
        anyhow::bail!("无效的数据库URL格式: {}", database_url)
    }
}

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // 从环境变量读取日志级别，默认为 info
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("正在启动 Agent Parallel System (Actix架构)...");

    // 初始化数据库连接
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost/agent_parallel_system".to_string()
    });

    // 先确保数据库存在
    ensure_database_exists(&database_url).await?;

    // 在创建连接池时添加配置
    let pool = PgPoolOptions::new()
        .max_connections(50) // 增加最大连接数
        .min_connections(5) // 最小连接数
        .acquire_timeout(std::time::Duration::from_secs(3)) // 获取连接超时
        .idle_timeout(std::time::Duration::from_secs(10)) // 空闲超时
        .connect(&database_url)
        .await?;
    info!("数据库连接成功");

    // 初始化数据库表结构

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_actor = crate::api::redis_actor::RedisActor::new(&redis_url).await?;
    let redis_addr = redis_actor.start();

    let persistence = DatabaseManager::new(pool.clone());
    persistence.initialize_database().await?;
    info!("数据库表结构初始化完成");

    // 消息通道管理器
    let channel_manager = ChannelManagerActor::new(pool.clone()).start();

    // 用户管理器Actor
    let user_manager = crate::api::user::actor_manager::UserManagerActor::new(pool.clone()).start();

    let mut open_aiconfig = OpenAIConfig::default();
    let api_base = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "null".to_string());
    open_aiconfig = open_aiconfig.with_api_base(api_base);
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "null".to_string());
    open_aiconfig = open_aiconfig.with_api_key(api_key);

    let prompt_template = CONFIG.chat_agent.prompt_template.clone();

    // 初始化聊天agent
    let chat_agent =
        ChatAgent::new(channel_manager.clone(), open_aiconfig, prompt_template).start();

    info!("终端接收器启动成功");

    info!("HTTP 服务器启动在 http://0.0.0.0:8000");
    info!("API 端点:");
    info!("  POST /auth/register - 用户注册");
    info!("  POST /auth/login    - 用户登录");
    info!("  POST /api/v1/refresh - 刷新Token");
    info!("  POST /api/v1/message  - 通用消息接口");

    // 启动 HTTP 服务器
    let server_result = HttpServer::new(move || {
        let cors = Cors::default()
            // 允许的来源 (开发环境下可以允许所有，生产环境建议指定具体域名)
            // .allow_any_origin() // 如果你想完全放开，可以使用这个
            .allowed_origin("http://localhost:5173") // 比如你的前端运行在 3000 端口
            .allowed_origin("http://127.0.0.1:5173")
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
            .app_data(web::Data::new(channel_manager.clone()))
            .app_data(web::Data::new(user_manager.clone()))
            .app_data(web::Data::new(redis_addr.clone()))
            .app_data(web::Data::new(chat_agent.clone()))
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
            .service(api::user::handler::login), // 最终路径: /auth/login
    );

    // --- 2. 受保护作用域 ---
    cfg.service(
        web::scope("/api/v1")
            .wrap(Auth) // 只对 /api/v1 下的路由生效
            .service(api::user::handler::refresh) // 你要求的：过期前刷新，所以放这里
            .route("/message", web::post().to(chat::handler::handle_message))
            .route(
                "/message",
                web::get().to(chat::handler::get_message_history),
            ),
    );
}
