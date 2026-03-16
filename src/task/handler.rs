use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};

use crate::task::dag_orchestrator::{DagOrchestrator, QueryAllTasks, SubmitTask};
use crate::task::model::{TaskItem, TaskInfo};

#[get("/tasks")]
pub async fn list_tasks_handler(
    dag_orchestrator: web::Data<Addr<DagOrchestrator>>,
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

#[post("/tasks")]
pub async fn create_task_handler(
    body: web::Json<TaskItem>,
    dag_orchestrator: web::Data<Addr<DagOrchestrator>>,
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
