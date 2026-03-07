use actix_web::{App, HttpResponse, HttpServer, web};
use dotenv::dotenv;
use tracing::{debug, info, error};
use async_openai::{
    types::chat::{
        CreateChatCompletionRequestArgs,
        ChatCompletionRequestSystemMessage,
    },
    Client,
    config::OpenAIConfig,
};
use std::env;
use std::sync::Arc;
use anyhow::Result;

// 引入通道层模块
mod channel;
use channel::{ChannelManager, ChannelBuilder, ReceiverFactory, configure_api_routes, MessagePersistence};
use channel::handler::{ChatHandler, TaskHandler, SystemHandler};
use channel::{Message, MessageSource, MessageType, MessagePriority};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // 从环境变量读取日志级别，默认为 info
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("正在启动 Agent Parallel System...");

    // 初始化数据库连接
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/agent_parallel_system".to_string());
    
    let pool = sqlx::PgPool::connect(&database_url).await?;
    info!("数据库连接成功");

    // 初始化数据库表结构
    let persistence = Arc::new(MessagePersistence::new(pool.clone()));
    persistence.initialize_database().await?;
    info!("数据库表结构初始化完成");

    // 初始化 OpenAI 客户端
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let base_url = env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    
    let config = OpenAIConfig::default()
        .with_api_key(api_key)
        .with_api_base(base_url);
    
    let client = Client::with_config(config);
    info!("OpenAI 客户端初始化成功");

    // 创建并启动通道层
    let channel_manager = Arc::new(
        ChannelBuilder::new()
            .with_max_queue_size(5000)
            .with_worker_threads(4)
            .with_message_timeout(300)
            .with_persistence(true)
            .build()
    );
    
    // 启动通道管理器
    channel_manager.start().await?;
    info!("通道层启动成功");

    // 注册消息处理器
    channel_manager.register_handler(Arc::new(ChatHandler::new())).await;
    channel_manager.register_handler(Arc::new(TaskHandler::new())).await;
    channel_manager.register_handler(Arc::new(SystemHandler::new())).await;
    info!("消息处理器注册完成");

    // 创建API接收器
    let api_receiver = ReceiverFactory::create_api_receiver(Arc::clone(&channel_manager));

    // 发送测试消息
    let test_message = Message::new(
        MessageSource::Api,
        MessageType::Chat,
        "test_user".to_string(),
        "这是一条测试消息".to_string(),
    ).with_priority(MessagePriority::Normal);

    channel_manager.send_message(test_message).await?;
    info!("测试消息已发送");

    // 创建终端接收器并启动交互模式
    let terminal_receiver = ReceiverFactory::create_terminal_receiver(Arc::clone(&channel_manager));
    terminal_receiver.start_interactive_mode().await?;
    info!("终端接收器启动成功");

    // 启动后台任务
    start_background_tasks(Arc::clone(&persistence), Arc::clone(&channel_manager)).await?;

    // 示例：创建一个简单的聊天完成请求
    if let Err(e) = openai_chat(&client).await {
        error!("OpenAI 测试失败: {}", e);
    }

    info!("HTTP 服务器启动在 http://0.0.0.0:8001");
    info!("API 端点:");
    info!("  POST /message  - 通用消息接口");
    info!("  POST /chat     - 聊天消息接口");
    info!("  POST /task     - 任务消息接口");
    info!("  POST /system   - 系统消息接口");

    // 启动 HTTP 服务器
    let server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Arc::clone(&api_receiver)))
            .app_data(web::Data::new(Arc::clone(&persistence)))
            .configure(|cfg| configure_api_routes(cfg, Arc::clone(&api_receiver)))
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health_check))
            .route("/stats", web::get().to(get_stats))
            .route("/test", web::get().to(test_endpoint))
    })
    .bind(("0.0.0.0", 8001))?
    .run()
    .await;
    
    // 停止通道层
    channel_manager.stop().await?;
    
    match server_result {
        Ok(_) => info!("服务器正常关闭"),
        Err(e) => error!("服务器错误: {}", e),
    }
    
    Ok(())
}

/// 启动后台任务
async fn start_background_tasks(
    persistence: Arc<MessagePersistence>,
    channel_manager: Arc<ChannelManager>,
) -> Result<()> {
    // 启动消息统计任务
    let stats_persistence = Arc::clone(&persistence);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 每5分钟
        
        loop {
            interval.tick().await;
            
            match stats_persistence.get_statistics(7).await {
                Ok(stats) => {
                    info!(
                        "消息统计 (7天): 总数={}, 成功={}, 失败={}, 成功率={:.1}%, 平均处理时间={:.2}s",
                        stats.total_messages,
                        stats.successful_messages,
                        stats.failed_messages,
                        stats.success_rate(),
                        stats.avg_processing_time_seconds
                    );
                }
                Err(e) => {
                    error!("获取消息统计失败: {}", e);
                }
            }
        }
    });

    // 启动清理任务
    let cleanup_persistence = Arc::clone(&persistence);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 每小时
        
        loop {
            interval.tick().await;
            
            match cleanup_persistence.cleanup_expired_messages().await {
                Ok(count) => {
                    if count > 0 {
                        info!("清理了 {} 条过期消息", count);
                    }
                }
                Err(e) => {
                    error!("清理过期消息失败: {}", e);
                }
            }
        }
    });

    Ok(())
}

/// 健康检查接口
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "agent-parallel-system"
    }))
}

/// 获取统计信息接口
async fn get_stats(
    persistence: web::Data<Arc<MessagePersistence>>,
) -> Result<HttpResponse, actix_web::Error> {
    match persistence.get_statistics(7).await {
        Ok(stats) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "total_messages": stats.total_messages,
            "successful_messages": stats.successful_messages,
            "failed_messages": stats.failed_messages,
            "success_rate": format!("{:.1}%", stats.success_rate()),
            "avg_processing_time_seconds": stats.avg_processing_time_seconds
        }))),
        Err(e) => {
            error!("获取统计信息失败: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "获取统计信息失败"
            })))
        }
    }
}

async fn openai_chat(client: &Client<OpenAIConfig>) -> Result<()> {
    info!("开始测试 OpenAI 聊天功能...");
    
    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(100u32)
        .model("deepseek-chat")
        .messages([
            ChatCompletionRequestSystemMessage::from("你是一个有用的助手。").into(),
        ])
        .build()?;

    let response = client.chat().create(request).await?;

    info!("OpenAI 响应:");
    for choice in response.choices {
        info!("角色: {}, 内容: {:?}", choice.message.role, choice.message.content);
    }
    
    Ok(())
}

async fn index() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Agent Parallel System",
        "endpoints": [
            "POST /message",
            "POST /chat",
            "POST /task",
            "POST /system",
            "GET /health",
            "GET /stats",
            "GET /test"
        ]
    }))
}

async fn test_endpoint() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "测试端点正常工作"
    }))
}
