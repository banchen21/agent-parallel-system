use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::Html,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    core::{errors::AppError, security::JwtUtil},
    models::{
        agent::{
            AgentHeartbeatRequest, AgentStatus, RegisterAgentRequest,
            TaskPriority as AgentTaskPriority,
        },
        message::SendMessageRequest,
        task::{
            CreateTaskRequest, TaskDecompositionRequest, TaskPriority, TaskStatus,
            UpdateTaskRequest, UpdateTaskStatusRequest,
        },
        user::{CreateUserRequest, LoginRequest},
        workflow::{CreateWorkflowRequest, ExecuteWorkflowRequest},
        workspace::{CreateWorkspaceRequest, GrantPermissionRequest, UpdateWorkspaceRequest},
    },
    AppState,
};

mod channel_routes;
mod chat_routes;

#[derive(Serialize)]
struct UiEndpoint {
    group: &'static str,
    method: &'static str,
    path: &'static str,
    description: &'static str,
    auth: bool,
    implemented: bool,
    request_example: Option<Value>,
    response_example: Value,
    notes: &'static str,
}

/// Web UI 路由
pub fn ui_routes() -> Router<AppState> {
    Router::new().route("/", get(ui_index))
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
        .route(
            "/tasks/:task_id",
            get(get_task).put(update_task).delete(delete_task),
        )
        .route("/tasks/:task_id/status", put(update_task_status))
        .route("/tasks/:task_id/decompose", post(decompose_task))
        .route("/tasks/:task_id/subtasks", get(get_subtasks))
}

/// 智能体路由
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(get_agents).post(register_agent))
        .route("/agents/:agent_id", get(get_agent).delete(delete_agent))
        .route("/agents/:agent_id/heartbeat", post(update_heartbeat))
        .route("/agents/:agent_id/status", put(update_agent_status))
        .route("/agents/:agent_id/assign-task", post(assign_task_to_agent))
        .route(
            "/agents/:agent_id/complete-task",
            post(complete_task_and_release_agent),
        )
        .route("/agents/stats", get(get_agent_stats))
}

/// 工作空间路由
pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces", post(create_workspace).get(get_workspaces))
        .route(
            "/workspaces/:workspace_id",
            get(get_workspace)
                .put(update_workspace)
                .delete(delete_workspace),
        )
        .route(
            "/workspaces/:workspace_id/permissions",
            get(get_workspace_permissions).post(grant_permission),
        )
        .route(
            "/workspaces/:workspace_id/permissions/:permission_id",
            delete(revoke_permission),
        )
        .route(
            "/workspaces/:workspace_id/documents",
            get(get_workspace_documents),
        )
        .route("/workspaces/:workspace_id/tools", get(get_workspace_tools))
        .route("/workspaces/:workspace_id/stats", get(get_workspace_stats))
}

/// 工作流路由（简化版）
pub fn workflow_routes() -> Router<AppState> {
    Router::new()
        .route("/workflows", get(get_workflows).post(create_workflow))
        .route(
            "/workflows/:workflow_id",
            get(get_workflow).delete(delete_workflow),
        )
        .route("/workflows/:workflow_id/execute", post(execute_workflow))
        .route(
            "/workflows/:workflow_id/executions",
            get(get_workflow_executions),
        )
        .route(
            "/workflows/:workflow_id/executions/:execution_id",
            get(get_workflow_execution),
        )
}

/// 聊天路由
pub fn chat_routes() -> Router<AppState> {
    Router::new()
        .route("/chat/sessions", post(chat_routes::create_chat_session))
        .route(
            "/chat/sessions/:channel_user_id",
            get(chat_routes::get_user_session),
        )
        .route(
            "/chat/sessions/:session_id",
            get(chat_routes::get_chat_session),
        )
        .route(
            "/chat/sessions/:session_id/messages",
            get(chat_routes::get_session_messages),
        )
        .route("/chat/messages", post(chat_routes::send_chat_message))
        .route(
            "/chat/sessions/:session_id/close",
            post(chat_routes::close_chat_session),
        )
}

/// 通道路由
pub fn channel_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/channels",
            post(channel_routes::create_channel_config).get(channel_routes::get_active_channels),
        )
        .route(
            "/channels/:config_id",
            get(channel_routes::get_channel_config).put(channel_routes::update_channel_config),
        )
        .route(
            "/channels/users/:user_id",
            get(channel_routes::get_channel_user),
        )
        .route(
            "/channels/users/:channel_user_id/bind/:system_user_id",
            post(channel_routes::bind_channel_user),
        )
        .route("/webhooks/telegram", post(channel_routes::telegram_webhook))
        .route("/webhooks/discord", post(channel_routes::discord_webhook))
}

/// 消息路由
pub fn message_routes() -> Router<AppState> {
    Router::new()
        .route("/messages", post(send_message))
        .route("/messages/user", get(get_my_messages))
        .route("/messages/user/unread-count", get(get_my_unread_count))
        .route("/messages/agent/:agent_id", get(get_agent_messages))
        .route("/messages/task/:task_id", get(get_task_messages))
        .route(
            "/messages/:message_type/:message_id/read",
            post(mark_message_read),
        )
        .route(
            "/messages/:message_type/:message_id",
            delete(delete_message),
        )
        .route(
            "/messages/:message_type/read-batch",
            post(mark_messages_read_batch),
        )
        .route(
            "/messages/:message_type/delete-batch",
            post(delete_messages_batch),
        )
        .route("/messages/broadcast", post(send_system_broadcast))
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

#[derive(Deserialize)]
struct RefreshTokenRequest {
    #[serde(alias = "refreshToken")]
    refresh_token: String,
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Deserialize)]
struct TaskListQuery {
    workspace_id: Uuid,
    status: Option<String>,
    priority: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Deserialize)]
struct WorkspaceListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Deserialize)]
struct WorkflowListQuery {
    workspace_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct ExecutionListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct MessageListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct BroadcastRequest {
    message_type: String,
    content: String,
    metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct BatchMessageIdsRequest {
    message_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
struct UpdateAgentStatusRequest {
    status: AgentStatus,
}

#[derive(Deserialize)]
struct AssignTaskBody {
    task_id: Uuid,
    priority: AgentTaskPriority,
    timeout: Option<i32>,
}

#[derive(Deserialize)]
struct CompleteTaskBody {
    task_id: Uuid,
    success: Option<bool>,
    result: Option<serde_json::Value>,
}

fn success_response(data: Value, message: &str) -> Json<Value> {
    Json(json!({
        "success": true,
        "message": message,
        "data": data,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

fn extract_user_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::AuthenticationError("未提供认证令牌".to_string()))?;

    if !auth.starts_with("Bearer ") {
        return Err(AppError::AuthenticationError("认证头格式错误".to_string()));
    }
    let token = auth.trim_start_matches("Bearer ").trim();
    if token.is_empty() {
        return Err(AppError::AuthenticationError("认证令牌为空".to_string()));
    }
    JwtUtil::extract_user_id_from_token(token)
}

fn parse_task_status(input: Option<String>) -> Option<TaskStatus> {
    input.map(TaskStatus::from)
}

fn parse_task_priority(input: Option<String>) -> Option<TaskPriority> {
    input.map(TaskPriority::from)
}

fn parse_priority_from_options(options: &serde_json::Value) -> TaskPriority {
    let priority = options
        .get("priority")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    parse_task_priority(priority).unwrap_or(TaskPriority::Medium)
}

// 认证处理器
async fn register(
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<Value>, AppError> {
    let user = state.auth_service.register(request).await?;
    Ok(success_response(json!(user), "注册成功"))
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<Value>, AppError> {
    let auth = state.auth_service.login(request).await?;
    Ok(success_response(json!(auth), "登录成功"))
}

async fn refresh_token(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<Value>, AppError> {
    let auth = state
        .auth_service
        .refresh_token(&request.refresh_token)
        .await?;
    Ok(success_response(json!(auth), "刷新令牌成功"))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state.auth_service.logout(user_id).await?;
    Ok(success_response(Value::Null, "登出成功"))
}

async fn get_current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let user = state.auth_service.get_current_user(user_id).await?;
    Ok(success_response(json!(user), "获取当前用户成功"))
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state
        .auth_service
        .change_password(user_id, &request.current_password, &request.new_password)
        .await?;
    Ok(success_response(Value::Null, "密码修改成功"))
}

// 任务处理器
async fn create_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let task = state.task_service.create_task(request, user_id).await?;

    // 创建后立即尝试自动分配，失败不影响任务创建结果
    if let Err(err) = state
        .orchestrator_service
        .assign_task_to_best_agent(task.id)
        .await
    {
        tracing::warn!("任务 {} 自动分配失败: {}", task.id, err);
    }

    let task_response = state
        .task_service
        .get_task_response(task.id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
    Ok(success_response(json!(task_response), "任务创建成功"))
}

async fn get_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TaskListQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let tasks = state
        .task_service
        .get_tasks_by_workspace(
            query.workspace_id,
            user_id,
            parse_task_status(query.status),
            parse_task_priority(query.priority),
            query.page,
            query.page_size,
        )
        .await?;

    let data: Vec<_> = tasks.into_iter().map(|t| t.to_response(vec![])).collect();
    Ok(success_response(json!(data), "任务列表获取成功"))
}

async fn get_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let task = state
        .task_service
        .get_task_response(task_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
    Ok(success_response(json!(task), "任务详情获取成功"))
}

async fn update_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    Json(request): Json<UpdateTaskRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let task = state
        .task_service
        .update_task(task_id, request, user_id)
        .await?;
    Ok(success_response(
        json!(task.to_response(vec![])),
        "任务更新成功",
    ))
}

async fn delete_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state.task_service.delete_task(task_id, user_id).await?;
    Ok(success_response(Value::Null, "任务删除成功"))
}

async fn update_task_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    Json(request): Json<UpdateTaskStatusRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let task = state
        .task_service
        .update_task_status(task_id, request, user_id)
        .await?;
    Ok(success_response(
        json!(task.to_response(vec![])),
        "任务状态更新成功",
    ))
}

async fn decompose_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    Json(request): Json<TaskDecompositionRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let result = state
        .task_service
        .decompose_task(task_id, request, user_id)
        .await?;
    Ok(success_response(json!(result), "任务分解成功"))
}

async fn get_subtasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let tasks = state.task_service.get_subtasks(task_id, user_id).await?;
    let data: Vec<_> = tasks.into_iter().map(|t| t.to_response(vec![])).collect();
    Ok(success_response(json!(data), "子任务获取成功"))
}

// 智能体处理器
async fn get_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let agents = state.agent_service.get_available_agents().await?;

    let data: Vec<Value> = agents
        .into_iter()
        .map(|a| {
            let capabilities: Vec<serde_json::Value> =
                serde_json::from_value(a.capabilities.clone()).unwrap_or_default();

            let success_rate = a
                .metadata
                .get("success_rate")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);

            json!({
                "id": a.id,
                "name": a.name,
                "description": a.description,
                "status": a.status.to_string(),
                "capabilities": capabilities,
                "current_load": a.current_load,
                "max_concurrent_tasks": a.max_concurrent_tasks,
                "success_rate": success_rate,
                "last_heartbeat": a.last_heartbeat_at,
                "created_at": a.created_at,
                "updated_at": a.updated_at,
            })
        })
        .collect();

    Ok(success_response(json!(data), "智能体列表获取成功"))
}

async fn register_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterAgentRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let agent = state.agent_service.register_agent(request).await?;
    Ok(success_response(
        json!(agent.to_response()),
        "智能体注册成功",
    ))
}

async fn get_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let agent = state
        .agent_service
        .get_agent_by_id(agent_id)
        .await?
        .ok_or_else(|| AppError::NotFound("智能体不存在".to_string()))?;
    Ok(success_response(
        json!(agent.to_response()),
        "智能体详情获取成功",
    ))
}

async fn delete_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    state.agent_service.delete_agent(agent_id).await?;
    Ok(success_response(json!({}), "智能体删除成功"))
}

async fn update_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<AgentHeartbeatRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let health = state
        .agent_service
        .update_heartbeat(agent_id, request)
        .await?;
    Ok(success_response(json!(health), "心跳更新成功"))
}

async fn update_agent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<UpdateAgentStatusRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let agent = state
        .agent_service
        .update_agent_status(agent_id, request.status)
        .await?;
    Ok(success_response(
        json!(agent.to_response()),
        "智能体状态更新成功",
    ))
}

async fn assign_task_to_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<AssignTaskBody>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let result = state
        .agent_service
        .assign_task_to_agent(crate::models::agent::TaskAssignmentRequest {
            task_id: request.task_id,
            agent_id,
            priority: request.priority,
            timeout: request.timeout,
        })
        .await?;
    Ok(success_response(json!(result), "任务分配成功"))
}

async fn complete_task_and_release_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<CompleteTaskBody>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let success = request.success.unwrap_or(true);
    state
        .orchestrator_service
        .handle_task_completion_by_agent(agent_id, request.task_id, request.result, success)
        .await?;
    Ok(success_response(Value::Null, "任务完成并释放智能体成功"))
}

async fn get_agent_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let stats = state.agent_service.get_agent_stats().await?;
    Ok(success_response(stats, "智能体统计获取成功"))
}

// 工作空间处理器
async fn create_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let workspace = state
        .workspace_service
        .create_workspace(request, user_id)
        .await?;
    let resp = state
        .workspace_service
        .get_workspace_response(workspace.id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
    Ok(success_response(json!(resp), "工作空间创建成功"))
}

async fn get_workspaces(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WorkspaceListQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let workspaces = state
        .workspace_service
        .get_user_workspaces(user_id, query.page, query.page_size)
        .await?;

    let mut responses = Vec::with_capacity(workspaces.len());
    for workspace in workspaces {
        if let Some(resp) = state
            .workspace_service
            .get_workspace_response(workspace.id, user_id)
            .await?
        {
            responses.push(resp);
        }
    }

    Ok(success_response(json!(responses), "工作空间列表获取成功"))
}

async fn get_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let workspace = state
        .workspace_service
        .get_workspace_response(workspace_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
    Ok(success_response(json!(workspace), "工作空间详情获取成功"))
}

async fn update_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<UpdateWorkspaceRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state
        .workspace_service
        .update_workspace(workspace_id, request, user_id)
        .await?;
    let workspace = state
        .workspace_service
        .get_workspace_response(workspace_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
    Ok(success_response(json!(workspace), "工作空间更新成功"))
}

async fn delete_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state
        .workspace_service
        .delete_workspace(workspace_id, user_id)
        .await?;
    Ok(success_response(Value::Null, "工作空间删除成功"))
}

async fn get_workspace_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let permissions = state
        .workspace_service
        .get_workspace_permissions(workspace_id, user_id)
        .await?;
    let data: Vec<_> = permissions.into_iter().map(|p| p.to_response()).collect();
    Ok(success_response(json!(data), "权限列表获取成功"))
}

async fn grant_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<GrantPermissionRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let permission = state
        .workspace_service
        .grant_permission(workspace_id, request, user_id)
        .await?;
    Ok(success_response(
        json!(permission.to_response()),
        "权限授予成功",
    ))
}

async fn revoke_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((_workspace_id, permission_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state
        .workspace_service
        .revoke_permission(permission_id, user_id)
        .await?;
    Ok(success_response(Value::Null, "权限撤销成功"))
}

async fn get_workspace_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let docs = state
        .workspace_service
        .get_workspace_documents(workspace_id, user_id)
        .await?;
    let data: Vec<_> = docs.into_iter().map(|d| d.to_response()).collect();
    Ok(success_response(json!(data), "文档列表获取成功"))
}

async fn get_workspace_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let tools = state
        .workspace_service
        .get_workspace_tools(workspace_id, user_id)
        .await?;
    let data: Vec<_> = tools.into_iter().map(|t| t.to_response()).collect();
    Ok(success_response(json!(data), "工具列表获取成功"))
}

async fn get_workspace_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let stats = state
        .workspace_service
        .get_workspace_stats(workspace_id, user_id)
        .await?;
    Ok(success_response(stats, "工作空间统计获取成功"))
}

// 工作流处理器
async fn get_workflows(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WorkflowListQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let workflows = state
        .workflow_service
        .list_workflows(user_id, query.workspace_id)
        .await?;
    let data: Vec<_> = workflows.into_iter().map(|w| w.to_response()).collect();
    Ok(success_response(json!(data), "工作流列表获取成功"))
}

async fn create_workflow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkflowRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let workflow = state
        .workflow_service
        .create_workflow(request, user_id)
        .await?;
    Ok(success_response(
        json!(workflow.to_response()),
        "工作流创建成功",
    ))
}

async fn get_workflow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let workflow = state
        .workflow_service
        .get_workflow(workflow_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("工作流不存在".to_string()))?;
    Ok(success_response(
        json!(workflow.to_response()),
        "工作流详情获取成功",
    ))
}

async fn delete_workflow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    state
        .workflow_service
        .delete_workflow(workflow_id, user_id)
        .await?;
    Ok(success_response(Value::Null, "工作流删除成功"))
}

async fn execute_workflow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
    Json(request): Json<ExecuteWorkflowRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;

    let workflow = state
        .workflow_service
        .get_workflow(workflow_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("工作流不存在".to_string()))?;

    if !workflow.is_active {
        return Err(AppError::ValidationError(
            "工作流已禁用，无法执行".to_string(),
        ));
    }

    let execution = state
        .workflow_service
        .create_execution(workflow_id, user_id, request.clone())
        .await?;

    let input = request.input.unwrap_or(json!({}));
    let options = request.options.unwrap_or(json!({}));

    let create_task_req = CreateTaskRequest {
        title: format!("Workflow: {} ({})", workflow.name, execution.id),
        description: workflow.description.clone(),
        priority: parse_priority_from_options(&options),
        workspace_id: workflow.workspace_id,
        requirements: json!({
            "workflow": workflow.definition,
            "capabilities": ["general_processing"]
        }),
        context: Some(json!({
            "workflow_id": workflow_id,
            "workflow_execution_id": execution.id,
            "input": input,
            "options": options
        })),
        metadata: Some(json!({
            "source": "workflow_execution"
        })),
    };

    let task = match state
        .task_service
        .create_task(create_task_req, user_id)
        .await
    {
        Ok(task) => task,
        Err(err) => {
            let _ = state
                .workflow_service
                .mark_execution_failed(execution.id, format!("{err}"))
                .await;
            return Err(err);
        }
    };

    let assigned = state
        .orchestrator_service
        .assign_task_to_best_agent(task.id)
        .await?;

    let updated_execution = state
        .workflow_service
        .mark_execution_dispatched(execution.id, task.id, assigned)
        .await?;

    Ok(success_response(
        json!({
            "execution": updated_execution.to_response(),
            "task_id": task.id,
            "assigned": assigned
        }),
        "工作流执行已触发",
    ))
}

async fn get_workflow_executions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
    Query(query): Query<ExecutionListQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let executions = state
        .workflow_service
        .list_executions(workflow_id, user_id, query.limit, query.offset)
        .await?;
    let data: Vec<_> = executions.into_iter().map(|e| e.to_response()).collect();
    Ok(success_response(json!(data), "工作流执行列表获取成功"))
}

async fn get_workflow_execution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((workflow_id, execution_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let execution = state
        .workflow_service
        .get_execution(workflow_id, execution_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("工作流执行记录不存在".to_string()))?;
    Ok(success_response(
        json!(execution.to_response()),
        "工作流执行详情获取成功",
    ))
}

async fn ensure_user_message_owner(
    state: &AppState,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let row = sqlx::query!(
        "SELECT user_id FROM user_messages WHERE id = $1",
        message_id
    )
    .fetch_optional(&state.db_pool)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound("消息不存在".to_string()))?;
    if row.user_id != user_id {
        return Err(AppError::PermissionDenied("没有权限操作该消息".to_string()));
    }
    Ok(())
}

// 消息处理器
async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;

    let response = match request.target_type.as_str() {
        "agent" => {
            let msg = state
                .message_service
                .send_agent_message(
                    request.target_id,
                    &request.message_type,
                    &request.content,
                    request.metadata,
                )
                .await?;
            json!(msg)
        }
        "task" => {
            let task = state
                .task_service
                .get_task_by_id(request.target_id, user_id)
                .await?;
            if task.is_none() {
                return Err(AppError::PermissionDenied(
                    "没有访问该任务的权限".to_string(),
                ));
            }

            let msg = state
                .message_service
                .send_task_message(
                    request.target_id,
                    &request.message_type,
                    &request.content,
                    request.metadata,
                )
                .await?;
            json!(msg)
        }
        "user" => {
            if request.target_id != user_id {
                return Err(AppError::PermissionDenied(
                    "只能给自己发送用户消息".to_string(),
                ));
            }
            let msg = state
                .message_service
                .send_user_message(
                    request.target_id,
                    &request.message_type,
                    &request.content,
                    request.metadata,
                )
                .await?;
            json!(msg)
        }
        "system" => {
            state
                .message_service
                .send_system_broadcast(&request.message_type, &request.content, request.metadata)
                .await?;
            Value::Null
        }
        _ => {
            return Err(AppError::ValidationError(
                "target_type 必须是 agent/task/user/system".to_string(),
            ));
        }
    };

    Ok(success_response(response, "消息发送成功"))
}

async fn get_my_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MessageListQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let messages = state
        .message_service
        .get_user_messages(user_id, query.limit, query.offset)
        .await?;
    Ok(success_response(json!(messages), "用户消息获取成功"))
}

async fn get_my_unread_count(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let unread_count = state.message_service.get_user_unread_count(user_id).await?;
    Ok(success_response(
        json!({ "unread_count": unread_count }),
        "未读消息数获取成功",
    ))
}

async fn get_agent_messages(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Query(query): Query<MessageListQuery>,
) -> Result<Json<Value>, AppError> {
    let messages = state
        .message_service
        .get_agent_messages(agent_id, query.limit, query.offset)
        .await?;
    Ok(success_response(json!(messages), "智能体消息获取成功"))
}

async fn get_task_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    Query(query): Query<MessageListQuery>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    let task = state.task_service.get_task_by_id(task_id, user_id).await?;
    if task.is_none() {
        return Err(AppError::PermissionDenied(
            "没有访问该任务的权限".to_string(),
        ));
    }

    let messages = state
        .message_service
        .get_task_messages(task_id, query.limit, query.offset)
        .await?;
    Ok(success_response(json!(messages), "任务消息获取成功"))
}

async fn mark_message_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((message_type, message_id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    if message_type == "user" {
        ensure_user_message_owner(&state, message_id, user_id).await?;
    }
    state
        .message_service
        .mark_message_as_read(message_id, &message_type)
        .await?;
    Ok(success_response(Value::Null, "消息已标记为已读"))
}

async fn delete_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((message_type, message_id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, AppError> {
    let user_id = extract_user_id(&headers)?;
    if message_type == "user" {
        ensure_user_message_owner(&state, message_id, user_id).await?;
    }
    state
        .message_service
        .delete_message(message_id, &message_type)
        .await?;
    Ok(success_response(Value::Null, "消息删除成功"))
}

async fn mark_messages_read_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(message_type): Path<String>,
    Json(request): Json<BatchMessageIdsRequest>,
) -> Result<Json<Value>, AppError> {
    if request.message_ids.is_empty() {
        return Err(AppError::ValidationError(
            "message_ids 不能为空".to_string(),
        ));
    }

    let user_id = extract_user_id(&headers)?;
    if message_type == "user" {
        for id in &request.message_ids {
            ensure_user_message_owner(&state, *id, user_id).await?;
        }
    }

    let affected = state
        .message_service
        .mark_messages_as_read_batch(&request.message_ids, &message_type)
        .await?;
    Ok(success_response(
        json!({ "affected_count": affected }),
        "批量标记已读成功",
    ))
}

async fn delete_messages_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(message_type): Path<String>,
    Json(request): Json<BatchMessageIdsRequest>,
) -> Result<Json<Value>, AppError> {
    if request.message_ids.is_empty() {
        return Err(AppError::ValidationError(
            "message_ids 不能为空".to_string(),
        ));
    }

    let user_id = extract_user_id(&headers)?;
    if message_type == "user" {
        for id in &request.message_ids {
            ensure_user_message_owner(&state, *id, user_id).await?;
        }
    }

    let affected = state
        .message_service
        .delete_messages_batch(&request.message_ids, &message_type)
        .await?;
    Ok(success_response(
        json!({ "affected_count": affected }),
        "批量删除消息成功",
    ))
}

async fn send_system_broadcast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BroadcastRequest>,
) -> Result<Json<Value>, AppError> {
    let _user_id = extract_user_id(&headers)?;
    state
        .message_service
        .send_system_broadcast(&request.message_type, &request.content, request.metadata)
        .await?;
    Ok(success_response(Value::Null, "系统广播发送成功"))
}

#[cfg(test)]
mod tests {
    use super::RefreshTokenRequest;

    #[test]
    fn refresh_request_supports_snake_case() {
        let payload = r#"{"refresh_token":"abc"}"#;
        let req: RefreshTokenRequest = serde_json::from_str(payload).unwrap();
        assert_eq!(req.refresh_token, "abc");
    }

    #[test]
    fn refresh_request_supports_camel_case_alias() {
        let payload = r#"{"refreshToken":"abc"}"#;
        let req: RefreshTokenRequest = serde_json::from_str(payload).unwrap();
        assert_eq!(req.refresh_token, "abc");
    }
}
