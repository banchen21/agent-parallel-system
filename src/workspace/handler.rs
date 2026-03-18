use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;
use serde_json::json;
use tracing::error;

use crate::{
    utils::handler_util::get_user_name,
    workspace::workspace_actor::{
        CreateWorkspace, DeleteWorkspace, GetWorkspaces, UpdateWorkspace, WorkspaceManageActor,
    },
};

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

// 查询
#[get("/workspace")]
async fn get_workspace_handler(
    workspace_manage_actor: web::Data<Addr<WorkspaceManageActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    // 发送消息给 Actor
    match workspace_manage_actor
        .send(GetWorkspaces(user_name.to_string()))
        .await
    {
        Ok(workspace) => match workspace {
            Ok(workspace) => HttpResponse::Ok().json(workspace),
            Err(e) => HttpResponse::BadRequest().body(e.to_string()),
        },
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[post("/workspace")]
async fn create_workspac_handler(
    create_workspace: web::Json<CreateWorkspace>,
    workspace_manage_actor: web::Data<Addr<WorkspaceManageActor>>,
) -> impl Responder {
    match workspace_manage_actor
        .send(create_workspace.into_inner())
        .await
    {
        Ok(workspace) => match workspace {
            Ok(workspace) => HttpResponse::Ok().json(workspace),
            Err(e) => HttpResponse::BadRequest().body(e.to_string()),
        },
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

// 删除
#[delete("/workspace/{name}")]
async fn delete_workspace_handler(
    name: web::Path<String>, // 2. 这里提取路径中的 {name}
    workspace_manage_actor: web::Data<Addr<WorkspaceManageActor>>,
) -> impl Responder {
    match workspace_manage_actor
        .send(DeleteWorkspace {
            name: name.into_inner(),
        })
        .await
    {
        Ok(res) => match res {
            // 3. 因为返回值是 Ok(())，我们手动构造一个友好的 JSON 响应
            Ok(_) => HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "工作区删除成功"
            })),
            Err(e) => {
                error!("删除工作区失败: {}", e);
                let e = json!({
                    "status": "error",
                    "message": format!("删除工作区失败: {}", e)
                });
                HttpResponse::BadRequest().json(e)
            }
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("Actor 通信异常: {}", e)),
    }
}

// 更新
#[put("/workspace/{name}")]
async fn update_workspace_handler(
    name: web::Path<String>,
    update_workspace: web::Json<UpdateWorkspaceRequest>,
    workspace_manage_actor: web::Data<Addr<WorkspaceManageActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    let payload = update_workspace.into_inner();
    match workspace_manage_actor
        .send(UpdateWorkspace {
            current_name: name.into_inner(),
            name: payload.name,
            description: payload.description,
            owner_username: user_name,
        })
        .await
    {
        Ok(Ok(workspace)) => HttpResponse::Ok().json(workspace),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(format!("Actor 通信异常: {}", e)),
    }
}
