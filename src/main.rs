use actix_cors::Cors;
use actix_web::http::header;
use actix_web::{App, HttpServer, web};
use anyhow::Result;
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tracing::{debug, error, info, warn};

mod channel;
mod core;
mod postgre_database;

// 聊天层消息处理
mod chat;

// 工作区
mod workspace;

mod agsnets;

// 任务层处理
mod task;

// neo4j图数据库记忆层
mod graph_memory;

// MCP 配置管理
mod mcp;

use crate::agsnets::actor_agents_manage::AgentManagerActor;
use crate::api::auth::Auth;
use crate::channel::actor_messages::ChannelManagerActor;
use crate::mcp::mcp_actor::McpAgentActor;
// 引入通道层模块
use crate::chat::chat_agent::ChatAgent;
use crate::chat::openai_actor::{OpenAIProxyActor, ProviderConfig};
use crate::core::actor_system::SysMonitorActor;
use crate::core::config::CONFIG;
use crate::core::handler::get_stats_handler;
use crate::graph_memory::actor_memory::AgentMemoryActor;
use crate::postgre_database::actor_database::DatabaseManager;
use crate::task::dag_orchestrator::DagOrchestrActor;
use crate::task::task_agent::TaskAgent;
use crate::utils::database_util::ensure_database_exists;
use crate::utils::env_util::env_var_or_default;
use crate::workspace::workspace_actor::WorkspaceManageActor;
use actix::{Actor, AsyncContext};
mod api;

// 工具
mod utils;
use chrono::Local;

#[actix_web::main]
async fn main() -> Result<()> {
    unsafe {
        std::env::set_var("TZ", "Asia/Shanghai");
    }

    dotenv().ok();

    // 从环境变量读取日志级别，默认为 info
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    crate::utils::log_broadcaster::init_broadcaster();
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(crate::utils::log_broadcaster::BroadcastLayer)
        .init();

    info!("正在启动 Agent Parallel System (Actix架构)...");

    info!("当前时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    // 系统监控
    let sys_monitor_actor = SysMonitorActor::new().start();

    let config = CONFIG.clone();

    // OpenAI 并行处理层（从 config/default.toml 的 [[providers]] 加载，支持多代理商）
    let timeout_secs = config.llm.timeout_secs;
    let max_tokens = config.llm.max_tokens;

    // 若 TOML 中未配置 [[providers]]，则回退到旧的单一 OPENAI_* 环境变量
    let open_aiproxy_actor = if config.providers.is_empty() {
        warn!("config/default.toml 中未配置 [[providers]]，回退到 OPENAI_* 环境变量");
        let fallback = ProviderConfig::new(
            env_var_or_default("OPENAI_PROVIDER_NAME", "default".to_string()),
            env_var_or_default("OPENAI_API_KEY", "null".to_string()),
            env_var_or_default("OPENAI_BASE_URL", "https://api.openai.com/v1".to_string()),
            env_var_or_default("OPENAI_MODEL", "gpt-3.5-turbo".to_string()),
        );
        OpenAIProxyActor::new(fallback, timeout_secs, max_tokens).start()
    } else {
        // 从配置构建代理商列表；api_key 优先读取环境变量 PROVIDER_{NAME大写}_API_KEY
        let build_provider = |item: &crate::core::config::ProviderItem| {
            let key_env = format!("PROVIDER_{}_API_KEY", item.name.to_uppercase());
            let api_key = env::var(&key_env).unwrap_or_else(|_| item.api_key.clone());
            info!("加载代理商: {} (model={})", item.name, item.default_model);
            ProviderConfig::new(&item.name, api_key, &item.base_url, &item.default_model)
        };

        let mut iter = config.providers.iter();
        // 确定默认代理商：若 llm.default_provider 非空则以其为准，否则用第一个
        let default_name = if config.llm.default_provider.is_empty() {
            config.providers[0].name.clone()
        } else {
            config.llm.default_provider.clone()
        };
        // 优先把 default_provider 放在最前面（作为 new() 的入参）
        let default_item = config
            .providers
            .iter()
            .find(|p| p.name == default_name)
            .unwrap_or(&config.providers[0]);

        let mut proxy =
            OpenAIProxyActor::new(build_provider(default_item), timeout_secs, max_tokens);
        for item in config
            .providers
            .iter()
            .filter(|p| p.name != default_item.name)
        {
            proxy = proxy.with_provider(build_provider(item));
        }
        proxy.start()
    };

    // postgresql
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost/agent_parallel_system".to_string()
    });
    debug!("数据库连接字符串: {}", database_url);
    ensure_database_exists(&database_url).await?;
    let pool = PgPoolOptions::new()
        .max_connections(50) // 增加最大连接数
        .min_connections(5) // 最小连接数
        .acquire_timeout(std::time::Duration::from_secs(3)) // 获取连接超时
        .idle_timeout(std::time::Duration::from_secs(10)) // 空闲超时
        .connect(&database_url)
        .await?;
    info!("PostgreSQL 连接成功");

    // neo4j 图数据库+智能记忆管理体
    let agent_memory_prompt = config.memory_agent.clone();
    let enable_memory_query = config.features.enable_memory_query;
    let neo4j_uri = env_var_or_default("NEO4J_URI", "127.0.0.1:7687".to_string());
    let neo4j_user = env_var_or_default("NEO4J_USERNAME", "neo4j".to_string());
    let neo4j_pass = env_var_or_default("NEO4J_PASSWORD", "Neo4j123456".to_string());

    let agent_memory_hactor = AgentMemoryActor::new(
        &neo4j_uri,
        &neo4j_user,
        &neo4j_pass,
        open_aiproxy_actor.clone(),
        agent_memory_prompt,
        enable_memory_query,
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

    // 初始化工作区
    let workspace_manage_actor = WorkspaceManageActor::new(pool.clone()).start();

    // MCP Agent
    let mcp_agent_prompt = config.mcp_agent.prompt.clone();
    let mcp_manager = McpAgentActor::new(pool.clone(), open_aiproxy_actor.clone(), mcp_agent_prompt).start();

    // 任务管理
    // 提前声明一个变量，用于将闭包内部创建的 dag_orchestrator 传递到外部
    let mut dag_orchestrator_opt = None;
    let submitted_recover_scan_interval_secs =
        config.task_review.submitted_recover_scan_interval_secs;
    let first_retry_delay_secs = config.task_review.first_retry_delay_secs;

    // 使用 Actor::create 解决循环依赖
    let agent_manager_actor = AgentManagerActor::create(|ctx| {
        let agent_manager_addr = ctx.address();
        let dag_orchestrator = DagOrchestrActor::new(
            pool.clone(),
            agent_manager_addr,
            channel_manager.clone(),
            workspace_manage_actor.clone(),
            submitted_recover_scan_interval_secs,
            first_retry_delay_secs,
        )
        .start();
        dag_orchestrator_opt = Some(dag_orchestrator.clone());

        AgentManagerActor::new(
            pool.clone(),
            config.agents.running_loop_interval_secs,
            open_aiproxy_actor.clone(),
            mcp_manager.clone(),
            dag_orchestrator,
        )
    });

    // 将 DagOrchestrator 从 Option 中解包出来
    let dag_orchestrator = dag_orchestrator_opt.expect("DagOrchestrator 启动失败");

    // 初始化任务Agent
    let task_agent_prompt = config.task_agent.prompt.clone();
    let task_agent = TaskAgent::new(
        open_aiproxy_actor.clone(),
        dag_orchestrator.clone(),
        workspace_manage_actor.clone(),
        task_agent_prompt,
    )
    .start();

    dag_orchestrator.do_send(crate::task::dag_orchestrator::RegisterTaskReviewer {
        task_agent: task_agent.clone(),
    });

    // 初始化聊天agent
    let chat_agent_prompt = config.chat_agent.prompt.clone();
    let chat_history_limit = config.limits.chat_history_limit;
    let chat_agent = ChatAgent::new(
        channel_manager.clone(),
        agent_memory_hactor_addr.clone(),
        open_aiproxy_actor.clone(),
        task_agent.clone(),
        chat_agent_prompt,
        chat_history_limit,
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
            .app_data(web::Data::new(dag_orchestrator.clone()))
            .app_data(web::Data::new(agent_manager_actor.clone()))
            .app_data(web::Data::new(agent_memory_hactor_addr.clone()))
            .app_data(web::Data::new(channel_manager.clone()))
            .app_data(web::Data::new(user_manager.clone()))
            .app_data(web::Data::new(redis_addr.clone()))
            .app_data(web::Data::new(chat_agent.clone()))
            .app_data(web::Data::new(sys_monitor_actor.clone()))
            .app_data(web::Data::new(workspace_manage_actor.clone()))
            .app_data(web::Data::new(mcp_manager.clone()))
            .app_data(web::Data::new(config.clone()))
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
    // --- 0. WebSocket（不经过 Auth 中间件，通过 URL 参数 token 自行验证）---
    cfg.service(chat::ws_handler::ws_chat_handler);
    // --- 0b. SSE 日志流（token 查询参数验证）---
    cfg.service(core::log_handler::log_stream_handler);

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
            // 聊天：HTTP 历史记录（发送消息已改为 WS 长连接）
            .service(chat::handler::get_message_history)
            // 工作空间
            .service(workspace::handler::get_workspace_handler)
            .service(workspace::handler::create_workspac_handler)
            .service(workspace::handler::delete_workspace_handler)
            // 任务（可选，后续打开）
            .service(task::handler::list_tasks_handler)
            .service(task::handler::get_task_handler)
            .service(task::handler::decide_task_review_handler)
            .service(task::handler::create_task_handler)
            .service(task::handler::delete_task_handler)
            // 智能体相关路由
            .service(agsnets::handler::list_agents_handler)
            .service(agsnets::handler::get_agent_provider_options_handler)
            .service(agsnets::handler::save_agent_provider_options_handler)
            .service(agsnets::handler::create_agent_handler)
            .service(agsnets::handler::start_agent_handler)
            .service(agsnets::handler::stop_agent_handler)
            .service(agsnets::handler::delete_agent_handler)
            // MCP 自建工具管理
            .service(mcp::handler::list_mcp_tools_handler)
            .service(mcp::handler::create_mcp_tool_handler)
            .service(mcp::handler::update_mcp_tool_handler)
            .service(mcp::handler::delete_mcp_tool_handler)
            // 记忆库 CRUD
            .service(graph_memory::handler::list_memory_nodes_handler)
            .service(graph_memory::handler::search_memory_nodes_handler)
            .service(graph_memory::handler::create_memory_node_handler)
            .service(graph_memory::handler::update_memory_node_handler)
            .service(graph_memory::handler::delete_memory_node_handler)
            .service(graph_memory::handler::list_node_relationships_handler)
            .service(graph_memory::handler::create_memory_relationship_handler)
            .service(graph_memory::handler::delete_memory_relationship_handler),
    );
}
