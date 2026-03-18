use actix::Addr;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use crate::chat::openai_actor::ChatAgentError;
use crate::graph_memory::actor_memory::{
    AgentMemoryActor, CreateMemoryNode, CreateMemoryRelationship, DeleteMemoryNode,
    DeleteMemoryRelationship, ListMemoryNodes, ListNodeRelationships, UpdateMemoryNode,
};

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub node_type: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub node_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRelationshipRequest {
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: String,
}

fn parse_id(raw: &str) -> Result<i64, ChatAgentError> {
    raw.parse::<i64>()
        .map_err(|_| ChatAgentError::QueryError("无效的ID".to_string()))
}

#[get("/memory/nodes")]
pub async fn list_memory_nodes_handler(
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    match memory_actor.send(ListMemoryNodes { query: None }).await {
        Ok(Ok(nodes)) => HttpResponse::Ok().json(nodes),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/memory/search")]
pub async fn search_memory_nodes_handler(
    query: web::Query<SearchQuery>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    match memory_actor
        .send(ListMemoryNodes {
            query: query.q.clone(),
        })
        .await
    {
        Ok(Ok(nodes)) => HttpResponse::Ok().json(nodes),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/memory/nodes")]
pub async fn create_memory_node_handler(
    req: web::Json<CreateNodeRequest>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    let req = req.into_inner();
    match memory_actor
        .send(CreateMemoryNode {
            name: req.name,
            description: req.description,
            node_type: req.node_type,
        })
        .await
    {
        Ok(Ok(node)) => HttpResponse::Ok().json(node),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("/memory/nodes/{node_id}")]
pub async fn update_memory_node_handler(
    path: web::Path<String>,
    req: web::Json<UpdateNodeRequest>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    let node_id = match parse_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };
    let req = req.into_inner();

    let name = req.name.unwrap_or_default();
    let description = req.description.unwrap_or_default();
    let node_type = req.node_type.unwrap_or_else(|| "concept".to_string());

    match memory_actor
        .send(UpdateMemoryNode {
            node_id,
            name,
            description,
            node_type,
        })
        .await
    {
        Ok(Ok(node)) => HttpResponse::Ok().json(node),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/memory/nodes/{node_id}")]
pub async fn delete_memory_node_handler(
    path: web::Path<String>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    let node_id = match parse_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    match memory_actor.send(DeleteMemoryNode { node_id }).await {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/memory/nodes/{node_id}/relationships")]
pub async fn list_node_relationships_handler(
    path: web::Path<String>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    let node_id = match parse_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    match memory_actor.send(ListNodeRelationships { node_id }).await {
        Ok(Ok(relationships)) => HttpResponse::Ok().json(relationships),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/memory/relationships")]
pub async fn create_memory_relationship_handler(
    req: web::Json<CreateRelationshipRequest>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    let req = req.into_inner();

    let source_id = match parse_id(&req.source_id) {
        Ok(v) => v,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };
    let target_id = match parse_id(&req.target_id) {
        Ok(v) => v,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    match memory_actor
        .send(CreateMemoryRelationship {
            source_id,
            target_id,
            relationship_type: req.relationship_type,
        })
        .await
    {
        Ok(Ok(rel)) => HttpResponse::Ok().json(rel),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/memory/relationships/{relationship_id}")]
pub async fn delete_memory_relationship_handler(
    path: web::Path<String>,
    memory_actor: web::Data<Addr<AgentMemoryActor>>,
) -> impl Responder {
    let relationship_id = match parse_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    match memory_actor
        .send(DeleteMemoryRelationship { relationship_id })
        .await
    {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
