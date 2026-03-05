use axum::{
    Json,
    response::Html,
    routing::{get, post, put, delete},
    Router,
};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
struct UiEndpoint {
    method: &'static str,
    path: &'static str,
    description: &'static str,
}

/// Web UI 路由
pub fn ui_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(ui_index))
        .route("/ui/endpoints", get(ui_endpoints))
}

/// 健康检查路由
pub fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
}

/// 认证路由
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_token))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(get_current_user))
        .route("/auth/change-password", post(change_password))
}

/// 任务路由
pub fn task_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", post(create_task).get(get_tasks))
        .route("/tasks/{task_id}", get(get_task).put(update_task).delete(delete_task))
        .route("/tasks/{task_id}/status", put(update_task_status))
        .route("/tasks/{task_id}/decompose", post(decompose_task))
        .route("/tasks/{task_id}/subtasks", get(get_subtasks))
}

/// 智能体路由
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(get_agents).post(register_agent))
        .route("/agents/{agent_id}", get(get_agent))
        .route("/agents/{agent_id}/heartbeat", post(update_heartbeat))
        .route("/agents/{agent_id}/status", put(update_agent_status))
        .route("/agents/{agent_id}/assign-task", post(assign_task_to_agent))
        .route("/agents/{agent_id}/complete-task", post(complete_task_and_release_agent))
        .route("/agents/stats", get(get_agent_stats))
}

/// 工作空间路由
pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces", post(create_workspace).get(get_workspaces))
        .route("/workspaces/{workspace_id}", get(get_workspace).put(update_workspace).delete(delete_workspace))
        .route("/workspaces/{workspace_id}/permissions", get(get_workspace_permissions).post(grant_permission))
        .route("/workspaces/{workspace_id}/permissions/{permission_id}", delete(revoke_permission))
        .route("/workspaces/{workspace_id}/documents", get(get_workspace_documents))
        .route("/workspaces/{workspace_id}/tools", get(get_workspace_tools))
        .route("/workspaces/{workspace_id}/stats", get(get_workspace_stats))
}

/// 工作流路由（简化版）
pub fn workflow_routes() -> Router<AppState> {
    Router::new()
        .route("/workflows", get(get_workflows).post(create_workflow))
        .route("/workflows/{workflow_id}", get(get_workflow).delete(delete_workflow))
        .route("/workflows/{workflow_id}/execute", post(execute_workflow))
        .route("/workflows/{workflow_id}/executions/{execution_id}", get(get_workflow_execution))
}

// 健康检查处理器
async fn health_check() -> &'static str {
    "OK"
}

async fn ready_check() -> &'static str {
    "READY"
}

async fn ui_index() -> Html<&'static str> {
    Html(include_str!("web_ui.html"))
}

async fn ui_endpoints() -> Json<Vec<UiEndpoint>> {
    Json(vec![
        UiEndpoint { method: "GET", path: "/health", description: "健康检查" },
        UiEndpoint { method: "GET", path: "/ready", description: "就绪检查" },
        UiEndpoint { method: "POST", path: "/auth/register", description: "用户注册（占位）" },
        UiEndpoint { method: "POST", path: "/auth/login", description: "用户登录（占位）" },
        UiEndpoint { method: "GET", path: "/tasks", description: "任务列表（占位）" },
        UiEndpoint { method: "POST", path: "/tasks", description: "创建任务（占位）" },
        UiEndpoint { method: "GET", path: "/agents", description: "智能体列表（占位）" },
        UiEndpoint { method: "GET", path: "/workspaces", description: "工作空间列表（占位）" },
        UiEndpoint { method: "GET", path: "/workflows", description: "工作流列表（占位）" },
    ])
}

// 认证处理器（占位符）
async fn register() -> &'static str {
    "Register endpoint"
}

async fn login() -> &'static str {
    "Login endpoint"
}

async fn refresh_token() -> &'static str {
    "Refresh token endpoint"
}

async fn logout() -> &'static str {
    "Logout endpoint"
}

async fn get_current_user() -> &'static str {
    "Get current user endpoint"
}

async fn change_password() -> &'static str {
    "Change password endpoint"
}

// 任务处理器（占位符）
async fn create_task() -> &'static str {
    "Create task endpoint"
}

async fn get_tasks() -> &'static str {
    "Get tasks endpoint"
}

async fn get_task() -> &'static str {
    "Get task endpoint"
}

async fn update_task() -> &'static str {
    "Update task endpoint"
}

async fn delete_task() -> &'static str {
    "Delete task endpoint"
}

async fn update_task_status() -> &'static str {
    "Update task status endpoint"
}

async fn decompose_task() -> &'static str {
    "Decompose task endpoint"
}

async fn get_subtasks() -> &'static str {
    "Get subtasks endpoint"
}

// 智能体处理器（占位符）
async fn get_agents() -> &'static str {
    "Get agents endpoint"
}

async fn register_agent() -> &'static str {
    "Register agent endpoint"
}

async fn get_agent() -> &'static str {
    "Get agent endpoint"
}

async fn update_heartbeat() -> &'static str {
    "Update heartbeat endpoint"
}

async fn update_agent_status() -> &'static str {
    "Update agent status endpoint"
}

async fn assign_task_to_agent() -> &'static str {
    "Assign task to agent endpoint"
}

async fn complete_task_and_release_agent() -> &'static str {
    "Complete task and release agent endpoint"
}

async fn get_agent_stats() -> &'static str {
    "Get agent stats endpoint"
}

// 工作空间处理器（占位符）
async fn create_workspace() -> &'static str {
    "Create workspace endpoint"
}

async fn get_workspaces() -> &'static str {
    "Get workspaces endpoint"
}

async fn get_workspace() -> &'static str {
    "Get workspace endpoint"
}

async fn update_workspace() -> &'static str {
    "Update workspace endpoint"
}

async fn delete_workspace() -> &'static str {
    "Delete workspace endpoint"
}

async fn get_workspace_permissions() -> &'static str {
    "Get workspace permissions endpoint"
}

async fn grant_permission() -> &'static str {
    "Grant permission endpoint"
}

async fn revoke_permission() -> &'static str {
    "Revoke permission endpoint"
}

async fn get_workspace_documents() -> &'static str {
    "Get workspace documents endpoint"
}

async fn get_workspace_tools() -> &'static str {
    "Get workspace tools endpoint"
}

async fn get_workspace_stats() -> &'static str {
    "Get workspace stats endpoint"
}

// 工作流处理器（占位符）
async fn get_workflows() -> &'static str {
    "Get workflows endpoint"
}

async fn create_workflow() -> &'static str {
    "Create workflow endpoint"
}

async fn get_workflow() -> &'static str {
    "Get workflow endpoint"
}

async fn delete_workflow() -> &'static str {
    "Delete workflow endpoint"
}

async fn execute_workflow() -> &'static str {
    "Execute workflow endpoint"
}

async fn get_workflow_execution() -> &'static str {
    "Get workflow execution endpoint"
}
