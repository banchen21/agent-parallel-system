use actix::Addr;
use actix_web::{delete, get, post, web, HttpResponse, Responder};
use serde_json::json;
use tracing::error;

use crate::mcp::mcp_actor::{AddMcpConfig, DeleteMcpConfig, GetMcpConfig, ListMcpConfigs, McpManagerActor};
use crate::mcp::model::McpConfig;

/// 添加 MCP 配置
#[post("/mcp")]
async fn add_mcp_handler(
    config: web::Json<McpConfig>,
    mcp_manager: web::Data<Addr<McpManagerActor>>,
) -> impl Responder {
    match mcp_manager
        .send(AddMcpConfig {
            config: config.into_inner(),
        })
        .await
    {
        Ok(result) => match result {
            Ok(config) => HttpResponse::Ok().json(config),
            Err(e) => {
                error!("添加 MCP 配置失败: {}", e);
                HttpResponse::BadRequest().json(json!({
                    "status": "error",
                    "message": e.to_string()
                }))
            }
        },
        Err(e) => {
            error!("Actor 通信失败: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Actor 通信失败: {}", e)
            }))
        }
    }
}

/// 删除 MCP 配置
#[delete("/mcp/{name}")]
async fn delete_mcp_handler(
    name: web::Path<String>,
    mcp_manager: web::Data<Addr<McpManagerActor>>,
) -> impl Responder {
    match mcp_manager
        .send(DeleteMcpConfig {
            name: name.into_inner(),
        })
        .await
    {
        Ok(result) => match result {
            Ok(_) => HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "MCP 配置删除成功"
            })),
            Err(e) => {
                error!("删除 MCP 配置失败: {}", e);
                HttpResponse::BadRequest().json(json!({
                    "status": "error",
                    "message": e.to_string()
                }))
            }
        },
        Err(e) => {
            error!("Actor 通信失败: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Actor 通信失败: {}", e)
            }))
        }
    }
}

/// 查询单个 MCP 配置
#[get("/mcp/{name}")]
async fn get_mcp_handler(
    name: web::Path<String>,
    mcp_manager: web::Data<Addr<McpManagerActor>>,
) -> impl Responder {
    match mcp_manager
        .send(GetMcpConfig {
            name: name.into_inner(),
        })
        .await
    {
        Ok(result) => match result {
            Ok(config) => HttpResponse::Ok().json(config),
            Err(e) => {
                error!("查询 MCP 配置失败: {}", e);
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "message": e.to_string()
                }))
            }
        },
        Err(e) => {
            error!("Actor 通信失败: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Actor 通信失败: {}", e)
            }))
        }
    }
}

/// 查询所有 MCP 配置
#[get("/mcp")]
async fn list_mcp_handler(
    mcp_manager: web::Data<Addr<McpManagerActor>>,
) -> impl Responder {
    match mcp_manager.send(ListMcpConfigs).await {
        Ok(result) => match result {
            Ok(configs) => HttpResponse::Ok().json(configs),
            Err(e) => {
                error!("查询 MCP 配置列表失败: {}", e);
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": e.to_string()
                }))
            }
        },
        Err(e) => {
            error!("Actor 通信失败: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Actor 通信失败: {}", e)
            }))
        }
    }
}
