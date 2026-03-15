use actix::Addr;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use crate::agsnets::actor_agents::{AgentManageActor, CreateAgent};

// 创建agent
#[post("/agent")]
async fn create_agent_handler(
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
