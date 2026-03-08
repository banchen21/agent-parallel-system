//! 测试ChatAgent流式响应debug输出的示例
//!
//! 运行方式: cargo run --example test_streaming_debug

// 注意：这是一个演示文件，展示如何使用debug宏测试流式响应
// 在实际使用中，需要根据项目结构调整导入路径

use actix::Actor;
use async_openai::config::OpenAIConfig;
use chrono::Utc;
use std::collections::HashMap;
use tracing_subscriber;
use uuid::Uuid;

// 模拟的导入路径 - 在实际项目中需要根据crate结构调整
// use crate::chat_handler::chat_agent::{ChatAgent, ProcessUserMessage, UserMessage};
// use crate::channel::actor_manager::ChannelManagerActor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统，显示debug级别日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    println!("🚀 开始测试ChatAgent流式响应debug输出...");

    // 创建OpenAI配置（需要设置真实的环境变量）
    let mut openai_config = OpenAIConfig::default();

    // 从环境变量读取配置
    let api_base = std::env::var("OPENAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let api_key =
        std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "your-api-key-here".to_string());

    openai_config = openai_config.with_api_base(api_base);
    openai_config = openai_config.with_api_key(api_key);

    // 注意：这里需要真实的ChannelManager，为了演示我们使用一个模拟的地址
    // 在实际使用中，您需要创建真实的ChannelManagerActor
    println!("⚠️  注意：此示例需要真实的数据库连接和ChannelManagerActor");
    println!("💡 在实际环境中，请确保设置了正确的OPENAI_API_KEY和OPENAI_BASE_URL环境变量");

    // 模拟的测试消息结构
    println!("📝 模拟测试消息: 请简单介绍一下Rust编程语言的特点");
    println!("📊 模拟消息ID: {}, 用户ID: test_user", Uuid::new_v4());

    // 在实际环境中，您需要：
    // 1. 创建真实的数据库连接池
    // 2. 创建ChannelManagerActor
    // 3. 创建ChatAgent并发送消息

    println!("\n🎯 Debug输出说明:");
    println!("🏗️  - ChatAgent实例创建");
    println!("🎬 - Actor启动和停止");
    println!("📨 - 接收用户消息");
    println!("🚀 - 开始流式响应处理");
    println!("📤 - 发送OpenAI请求");
    println!("📡 - 接收流式响应");
    println!("📦 - 每个数据块的处理");
    println!("📝 - 流式内容片段");
    println!("📊 - 累积内容长度");
    println!("🎉 - 流式响应完成");
    println!("🎯 - 最终响应创建");

    println!("\n📋 实际使用示例:");
    println!("```rust");
    println!("// 设置debug日志级别");
    println!("tracing_subscriber::fmt()");
    println!("    .with_max_level(tracing::Level::DEBUG)");
    println!("    .init();");
    println!("");
    println!("// 创建ChatAgent并发送消息");
    println!("let chat_agent = ChatAgent::new(channel_manager, openai_config).start();");
    println!("let response = chat_agent.send(ProcessUserMessage {{");
    println!("    user_message,");
    println!("    session_id: Some(\"session123\".to_string()),");
    println!("}}).await?;");
    println!("```");

    println!("\n✅ 示例准备完成！在实际环境中运行时将看到详细的debug输出。");

    Ok(())
}
