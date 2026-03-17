use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use crate::task::dag_orchestrator::{
    DagOrchestrActor, DeleteTaskById, QueryAllTasks, QueryTaskDetailById, ResolveTaskReviewDecision, SubmitTask,
};
use crate::task::model::TaskItem;

#[derive(Debug, Deserialize)]
pub struct ReviewDecisionRequest {
    pub accept: bool,
}

#[get("/tasks")]
pub async fn list_tasks_handler(
    dag_orchestrator: web::Data<Addr<DagOrchestrActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    match dag_orchestrator.send(QueryAllTasks(user_name)).await {
        Ok(Ok(tasks)) => HttpResponse::Ok().json(tasks),
        Ok(Err(e)) => HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/tasks/{task_id}")]
pub async fn get_task_handler(
    path: web::Path<uuid::Uuid>,
    dag_orchestrator: web::Data<Addr<DagOrchestrActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match dag_orchestrator
        .send(QueryTaskDetailById {
            task_id: path.into_inner(),
            user_name,
        })
        .await
    {
        Ok(Ok(task)) => HttpResponse::Ok().json(task),
        Ok(Err(e)) if e.to_string().contains("not found") => HttpResponse::NotFound().body(e.to_string()),
        Ok(Err(e)) => HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/tasks/{task_id}/review-decision")]
pub async fn decide_task_review_handler(
    path: web::Path<uuid::Uuid>,
    body: web::Json<ReviewDecisionRequest>,
    dag_orchestrator: web::Data<Addr<DagOrchestrActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match dag_orchestrator
        .send(ResolveTaskReviewDecision {
            task_id: path.into_inner(),
            user_name,
            accept: body.accept,
        })
        .await
    {
        Ok(Ok(task)) => HttpResponse::Ok().json(task),
        Ok(Err(e)) if e.to_string().contains("not found") => HttpResponse::NotFound().body(e.to_string()),
        Ok(Err(e)) if e.to_string().contains("not under review") => HttpResponse::BadRequest().body(e.to_string()),
        Ok(Err(e)) => HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/tasks")]
pub async fn create_task_handler(
    body: web::Json<TaskItem>,
    dag_orchestrator: web::Data<Addr<DagOrchestrActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    let task = body.into_inner();
    match dag_orchestrator
        .send(SubmitTask {
            user_name: user_name.clone(),
            task,
        })
        .await
    {
        Ok(()) => HttpResponse::Accepted().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/tasks/{task_id}")]
pub async fn delete_task_handler(
    path: web::Path<uuid::Uuid>,
    dag_orchestrator: web::Data<Addr<DagOrchestrActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match dag_orchestrator
        .send(DeleteTaskById {
            task_id: path.into_inner(),
            user_name,
        })
        .await
    {
        Ok(Ok(())) => HttpResponse::NoContent().finish(),
        Ok(Err(e)) if e.to_string().contains("not found") => {
            HttpResponse::NotFound().body(e.to_string())
        }
        Ok(Err(e)) if e.to_string().contains("only completed") => {
            HttpResponse::BadRequest().body(e.to_string())
        }
        Ok(Err(e)) => HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
