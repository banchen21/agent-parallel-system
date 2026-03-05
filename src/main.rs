use std::{net::SocketAddr, sync::Arc};

use axum::{
    http::{HeaderValue, Method},
    Router,
};
use clap::{Parser, Subcommand};
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing::{info};

use agent_parallel_system::{
    api::routes,
    core::{config, database, logging, realtime_logging},
    workers::task_worker,
};

/// 基于LLM的多智能体并行协作系统
#[derive(Parser)]
#[command(name = "agent-parallel-system")]
#[command(about = "基于LLM的多智能体并行协作系统", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>, 
}

#[derive(Subcommand)]
enum Commands {
    /// 启动API服务器
    Server,
    /// 启动后台工作器
    Worker,
    /// 运行数据库迁移
    Migrate,
    /// 显示系统信息
    Info,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    // 初始化日志
    logging::init_logging();
    
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Server) => {
            start_server().await?;
        }
        Some(Commands::Worker) => {
            start_worker().await?;
        }
        Some(Commands::Migrate) => {
            run_migrations().await?;
        }
        Some(Commands::Info) => {
            show_info().await?;
        }
        None => {
            // 默认启动API服务器
            start_server().await?;
        }
    }
    
    Ok(())
}

/// 启动API服务器
async fn start_server() -> anyhow::Result<()> {
    info!(
        "Starting {} v{} in {} environment",
        config::CONFIG.app_name,
        env!("CARGO_PKG_VERSION"),
        config::CONFIG.environment
    );
    
    // 初始化数据库连接池
    let db_pool = database::create_db_pool().await?;
    info!("Database connection pool created successfully");
    
    // 初始化Redis连接池
    let redis_pool = database::create_redis_pool().await?;
    info!("Redis connection pool created successfully");
    
    // 创建应用状态
    let app_state = agent_parallel_system::AppState::new(db_pool.clone(), redis_pool.clone());
    
    // 初始化实时日志管理器（简化版本）
    let realtime_log_manager = Arc::new(realtime_logging::RealtimeLogManager::new(
        redis_pool.clone(),
        db_pool.clone(),
    ));
    let app_state = app_state.with_realtime_log_manager(realtime_log_manager);
    
    // 构建 API 路由：同时挂载到根路径和配置前缀（兼容旧客户端）
    let api_routes = Router::new()
        .merge(routes::ui_routes())
        .merge(routes::health_routes())
        .merge(routes::auth_routes())
        .merge(routes::task_routes())
        .merge(routes::agent_routes())
        .merge(routes::workspace_routes())
        .merge(routes::workflow_routes())
        .merge(routes::message_routes());
        // 实时日志路由暂时禁用，等待完整实现
        // .merge(routes::realtime_log_routes());

    let api_prefix = config::CONFIG.server.api_prefix.clone();
    let app = Router::new()
        .merge(api_routes.clone())
        .nest(&api_prefix, api_routes)
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                .allow_headers(tower_http::cors::Any)
        )
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);
    
    // 启动服务器
    let addr = SocketAddr::from((
        config::CONFIG.server.host.parse::<std::net::IpAddr>()?,
        config::CONFIG.server.port,
    ));
    
    info!("Server listening on http://{}", addr);
    info!("API prefix mounted at {}", api_prefix);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// 启动后台工作器
async fn start_worker() -> anyhow::Result<()> {
    info!(
        "Starting worker for {} v{} in {} environment",
        config::CONFIG.app_name,
        env!("CARGO_PKG_VERSION"),
        config::CONFIG.environment
    );
    
    // 初始化数据库连接池
    let db_pool = database::create_db_pool().await?;
    info!("Database connection pool created successfully");
    
    // 初始化Redis连接池
    let redis_pool = database::create_redis_pool().await?;
    info!("Redis connection pool created successfully");
    
    // 启动任务工作器
    task_worker::start_worker(db_pool, redis_pool).await?;
    
    Ok(()) 
}

/// 运行数据库迁移
async fn run_migrations() -> anyhow::Result<()> {
    info!("Running database migrations...");
    
    // 初始化数据库连接池 
    let _db_pool = database::create_db_pool().await?;
    
    // 这里可以添加具体的迁移逻辑
    // 目前我们使用Docker Compose来运行SQL文件
    info!("Migrations will be applied by Docker Compose during startup");
    
    Ok(())
}

/// 显示系统信息
async fn show_info() -> anyhow::Result<()> {
    println!("基于LLM的多智能体并行协作系统");
    println!("版本: {}", env!("CARGO_PKG_VERSION"));
    println!("环境: {}", config::CONFIG.environment);
    println!("应用名称: {}", config::CONFIG.app_name);
    println!("服务器地址: {}:{}", config::CONFIG.server.host, config::CONFIG.server.port);
    println!("API前缀: {}", config::CONFIG.server.api_prefix);
    println!("数据库URL: {}", config::CONFIG.database.url);
    println!("Redis URL: {}", config::CONFIG.redis.url);
    
    Ok(())
}
