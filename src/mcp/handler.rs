use actix::Addr;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use serde_json::json;
use tracing::error;

use crate::mcp::mcp_actor::{DeleteMcpTool, ListMcpTools, McpAgentActor, UpsertMcpTool};
use crate::mcp::model::McpToolDefinition;

#[get("/mcp/tools")]
pub async fn list_mcp_tools_handler(mcp_actor: web::Data<Addr<McpAgentActor>>) -> impl Responder {
	match mcp_actor.send(ListMcpTools).await {
		Ok(Ok(tools)) => HttpResponse::Ok().json(tools),
		Ok(Err(e)) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
		Err(e) => {
			error!("MCP actor mailbox error when listing tools: {}", e);
			HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
		}
	}
}

#[post("/mcp/tools")]
pub async fn create_mcp_tool_handler(
	mcp_actor: web::Data<Addr<McpAgentActor>>,
	body: web::Json<McpToolDefinition>,
) -> impl Responder {
	match mcp_actor
		.send(UpsertMcpTool {
			tool: body.into_inner(),
		})
		.await
	{
		Ok(Ok(tool)) => HttpResponse::Ok().json(tool),
		Ok(Err(e)) => HttpResponse::BadRequest().json(json!({ "error": e.to_string() })),
		Err(e) => {
			error!("MCP actor mailbox error when creating tool: {}", e);
			HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
		}
	}
}

#[put("/mcp/tools/{tool_id}")]
pub async fn update_mcp_tool_handler(
	mcp_actor: web::Data<Addr<McpAgentActor>>,
	tool_id: web::Path<String>,
	body: web::Json<McpToolDefinition>,
) -> impl Responder {
	let mut tool = body.into_inner();
	tool.tool_id = tool_id.into_inner();

	match mcp_actor.send(UpsertMcpTool { tool }).await {
		Ok(Ok(saved)) => HttpResponse::Ok().json(saved),
		Ok(Err(e)) => HttpResponse::BadRequest().json(json!({ "error": e.to_string() })),
		Err(e) => {
			error!("MCP actor mailbox error when updating tool: {}", e);
			HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
		}
	}
}

#[delete("/mcp/tools/{tool_id}")]
pub async fn delete_mcp_tool_handler(
	mcp_actor: web::Data<Addr<McpAgentActor>>,
	tool_id: web::Path<String>,
) -> impl Responder {
	match mcp_actor
		.send(DeleteMcpTool {
			tool_id: tool_id.into_inner(),
		})
		.await
	{
		Ok(Ok(())) => HttpResponse::NoContent().finish(),
		Ok(Err(e)) => HttpResponse::BadRequest().json(json!({ "error": e.to_string() })),
		Err(e) => {
			error!("MCP actor mailbox error when deleting tool: {}", e);
			HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
		}
	}
}
