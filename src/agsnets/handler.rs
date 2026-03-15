use actix::Addr;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;
use uuid::Uuid;

use crate::agsnets::actor_agents::{
    AgentManageActor, AgentStatus, CreateAgent, DeleteAgent, GetAgent, ListAgentStatuses,
    ListAgents, QueryAgentStatus, UpdateAgentStatus,
};

// 创建 agent
#[post("/agent")]
pub async fn create_agent_handler(
    create_agent: web::Json<CreateAgent>,
    agsnets: web::Data<Addr<AgentManageActor>>,
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

// 查询 agent 列表  GET /api/v1/agent
#[get("/agent")]
pub async fn list_agents_handler(
    agsnets: web::Data<Addr<AgentManageActor>>,
) -> impl Responder {
    match agsnets.send(ListAgents).await {
        Ok(Ok(list)) => HttpResponse::Ok().json(list),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

// 查询单个 agent 详情  GET /api/v1/agent/{id}
#[get("/agent/{id}")]
pub async fn get_agent_handler(
    path: web::Path<Uuid>,
    agsnets: web::Data<Addr<AgentManageActor>>,
) -> impl Responder {
    match agsnets
        .send(GetAgent {
            agent_id: path.into_inner(),
        })
        .await
    {
        Ok(Ok(agent)) => HttpResponse::Ok().json(agent),
        Ok(Err(e)) => HttpResponse::NotFound().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

// 删除 agent  DELETE /api/v1/agent/{id}
#[delete("/agent/{id}")]
pub async fn delete_agent_handler(
    path: web::Path<Uuid>,
    agsnets: web::Data<Addr<AgentManageActor>>,
) -> impl Responder {
    match agsnets
        .send(DeleteAgent {
            agent_id: path.into_inner(),
        })
        .await
    {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "message": "智能体已删除" })),
        Ok(Err(e)) => HttpResponse::NotFound().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

// 查询单个 agent 状态  GET /api/v1/agent/{id}/status
#[get("/agent/{id}/status")]
pub async fn get_agent_status_handler(
    path: web::Path<Uuid>,
    agsnets: web::Data<Addr<AgentManageActor>>,
) -> impl Responder {
    let agent_id = path.into_inner();
    match agsnets.send(QueryAgentStatus { agent_id }).await {
        Ok(Ok(info)) => HttpResponse::Ok().json(info),
        Ok(Err(e)) => HttpResponse::NotFound().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

// 列出所有 agent 状态  GET /api/v1/agents/statuses
#[get("/agents/statuses")]
pub async fn list_agent_statuses_handler(
    agsnets: web::Data<Addr<AgentManageActor>>,
) -> impl Responder {
    match agsnets.send(ListAgentStatuses).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

// 更新 agent 状态  PUT /api/v1/agent/{id}/status
#[derive(Deserialize)]
struct UpdateStatusBody {
    status: AgentStatus,
}

#[put("/agent/{id}/status")]
pub async fn update_agent_status_handler(
    path: web::Path<Uuid>,
    body: web::Json<UpdateStatusBody>,
    agsnets: web::Data<Addr<AgentManageActor>>,
) -> impl Responder {
    let agent_id = path.into_inner();
    let status = body.into_inner().status;
    match agsnets
        .send(UpdateAgentStatus { agent_id, status })
        .await
    {
        Ok(Ok(())) => HttpResponse::Ok().body("状态已更新"),
        Ok(Err(e)) => HttpResponse::NotFound().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
