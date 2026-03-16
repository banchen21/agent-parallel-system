use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;
use uuid::Uuid;

use crate::agsnets::actor_agents::{AgentManagerActor, AgentStatus, CreateAgent, ListAgents};
use sqlx::Row;

// 创建 agent
#[post("/agent")]
pub async fn create_agent_handler(
    create_agent: web::Json<CreateAgent>,
    agsnets: web::Data<Addr<AgentManagerActor>>,
) -> impl Responder {
    let create_agent = create_agent.into_inner();
    match agsnets.send(create_agent.clone()).await {
        Ok(s) => match s {
            Ok(_info) => HttpResponse::Ok().json(_info),
            Err(e) => HttpResponse::BadRequest().body(e.to_string()),
        },
        Err(_e) => HttpResponse::InternalServerError().body(_e.to_string()),
    }
}

// 获取 agent 列表
#[get("/agent")]
pub async fn list_agents_handler(
    agsnets: web::Data<Addr<AgentManagerActor>>,
    req: HttpRequest,
) -> impl Responder {
    // 从请求上下文中获取当前用户名（由 Auth middleware 放入 extensions）
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match agsnets.send(ListAgents { user_name }).await {
        Ok(Ok(list)) => HttpResponse::Ok().json(list),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
