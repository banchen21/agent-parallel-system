use actix::Addr;
use actix_web::{HttpResponse, Responder, get, post, web};
use uuid::Uuid;

use crate::task_handler::actor_task::{CreateTask, CreateTaskInput, DagOrchestrator, GetTask, ListTasks};

#[get("/tasks")]
pub async fn list_tasks_handler(
    dag_orchestrator: web::Data<Addr<DagOrchestrator>>,
) -> impl Responder {
    match dag_orchestrator.send(ListTasks).await {
        Ok(tasks) => HttpResponse::Ok().json(tasks),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/tasks/{id}")]
pub async fn get_task_handler(
    path: web::Path<Uuid>,
    dag_orchestrator: web::Data<Addr<DagOrchestrator>>,
) -> impl Responder {
    match dag_orchestrator
        .send(GetTask {
            task_id: path.into_inner(),
        })
        .await
    {
        Ok(Some(task)) => HttpResponse::Ok().json(task),
        Ok(None) => HttpResponse::NotFound().body("任务不存在"),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/tasks")]
pub async fn create_task_handler(
    body: web::Json<CreateTaskInput>,
    dag_orchestrator: web::Data<Addr<DagOrchestrator>>,
) -> impl Responder {
    match dag_orchestrator
        .send(CreateTask {
            input: body.into_inner(),
        })
        .await
    {
        Ok(task) => HttpResponse::Ok().json(task),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}