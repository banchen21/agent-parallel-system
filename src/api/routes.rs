use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    Json,
    response::Html,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    core::{errors::AppError, security::JwtUtil},
    models::{
        agent::{
            AgentHeartbeatRequest, AgentStatus, RegisterAgentRequest, TaskPriority as AgentTaskPriority,
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

#[derive(Serialize)]
struct UiSpec {
    name: &'static str,
    version: &'static str,
    base_url: &'static str,
    generated_at: String,
    endpoints: Vec<UiEndpoint>,
}

/// Web UI 路由
pub fn ui_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(ui_index))
        .route("/docs", get(ui_index))
        .route("/ui/endpoints", get(ui_endpoints))
        .route("/ui/spec", get(ui_spec))
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
        .route("/tasks/:task_id", get(get_task).put(update_task).delete(delete_task))
        .route("/tasks/:task_id/status", put(update_task_status))
        .route("/tasks/:task_id/decompose", post(decompose_task))
        .route("/tasks/:task_id/subtasks", get(get_subtasks))
}

/// 智能体路由
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(get_agents).post(register_agent))
        .route("/agents/:agent_id", get(get_agent))
        .route("/agents/:agent_id/heartbeat", post(update_heartbeat))
        .route("/agents/:agent_id/status", put(update_agent_status))
        .route("/agents/:agent_id/assign-task", post(assign_task_to_agent))
        .route("/agents/:agent_id/complete-task", post(complete_task_and_release_agent))
        .route("/agents/stats", get(get_agent_stats))
}

/// 工作空间路由
pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces", post(create_workspace).get(get_workspaces))
        .route("/workspaces/:workspace_id", get(get_workspace).put(update_workspace).delete(delete_workspace))
        .route("/workspaces/:workspace_id/permissions", get(get_workspace_permissions).post(grant_permission))
        .route("/workspaces/:workspace_id/permissions/:permission_id", delete(revoke_permission))
        .route("/workspaces/:workspace_id/documents", get(get_workspace_documents))
        .route("/workspaces/:workspace_id/tools", get(get_workspace_tools))
        .route("/workspaces/:workspace_id/stats", get(get_workspace_stats))
}

/// 工作流路由（简化版）
pub fn workflow_routes() -> Router<AppState> {
    Router::new()
        .route("/workflows", get(get_workflows).post(create_workflow))
        .route("/workflows/:workflow_id", get(get_workflow).delete(delete_workflow))
        .route("/workflows/:workflow_id/execute", post(execute_workflow))
        .route("/workflows/:workflow_id/executions", get(get_workflow_executions))
        .route("/workflows/:workflow_id/executions/:execution_id", get(get_workflow_execution))
}

/// 消息路由
pub fn message_routes() -> Router<AppState> {
    Router::new()
        .route("/messages", post(send_message))
        .route("/messages/user", get(get_my_messages))
        .route("/messages/user/unread-count", get(get_my_unread_count))
        .route("/messages/agent/:agent_id", get(get_agent_messages))
        .route("/messages/task/:task_id", get(get_task_messages))
        .route("/messages/:message_type/:message_id/read", post(mark_message_read))
        .route("/messages/:message_type/:message_id", delete(delete_message))
        .route("/messages/:message_type/read-batch", post(mark_messages_read_batch))
        .route("/messages/:message_type/delete-batch", post(delete_messages_batch))
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

async fn ui_endpoints() -> Json<Vec<UiEndpoint>> {
    Json(build_ui_endpoints())
}

async fn ui_spec() -> Json<UiSpec> {
    Json(UiSpec {
        name: "Agent Parallel System",
        version: env!("CARGO_PKG_VERSION"),
        base_url: "/",
        generated_at: chrono::Utc::now().to_rfc3339(),
        endpoints: build_ui_endpoints(),
    })
}

fn build_ui_endpoints() -> Vec<UiEndpoint> {
    vec![
        UiEndpoint {
            group: "System",
            method: "GET",
            path: "/health",
            description: "健康检查",
            auth: false,
            implemented: true,
            request_example: None,
            response_example: json!("OK"),
            notes: "用于负载均衡器/网关存活探测。",
        },
        UiEndpoint {
            group: "System",
            method: "GET",
            path: "/ready",
            description: "就绪检查",
            auth: false,
            implemented: true,
            request_example: None,
            response_example: json!("READY"),
            notes: "用于启动后依赖检查通过的就绪探测。",
        },
        UiEndpoint {
            group: "Auth",
            method: "POST",
            path: "/auth/register",
            description: "用户注册",
            auth: false,
            implemented: true,
            request_example: Some(json!({
                "username": "alice_01",
                "email": "alice@example.com",
                "password": "ChangeMe#123",
                "first_name": "Alice",
                "last_name": "Chen"
            })),
            response_example: json!({"message": "Register endpoint"}),
            notes: "当前为占位处理器，后续将接入 auth_service。",
        },
        UiEndpoint {
            group: "Auth",
            method: "POST",
            path: "/auth/login",
            description: "用户登录",
            auth: false,
            implemented: true,
            request_example: Some(json!({
                "username": "alice_01",
                "password": "ChangeMe#123"
            })),
            response_example: json!({"message": "Login endpoint"}),
            notes: "当前为占位处理器，后续将返回 JWT。",
        },
        UiEndpoint {
            group: "Auth",
            method: "POST",
            path: "/auth/refresh",
            description: "刷新访问令牌",
            auth: true,
            implemented: true,
            request_example: Some(json!({ "refresh_token": "refresh-token-value" })),
            response_example: json!({"message": "Refresh token endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Auth",
            method: "POST",
            path: "/auth/logout",
            description: "用户登出",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Logout endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Auth",
            method: "GET",
            path: "/auth/me",
            description: "获取当前用户",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get current user endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Auth",
            method: "POST",
            path: "/auth/change-password",
            description: "修改密码",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "old_password": "OldPass#123",
                "new_password": "NewPass#456"
            })),
            response_example: json!({"message": "Change password endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Task",
            method: "POST",
            path: "/tasks",
            description: "创建任务",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "title": "分析 Q1 销售数据",
                "description": "生成趋势结论与改进建议",
                "priority": "medium",
                "workspace_id": "00000000-0000-0000-0000-000000000001",
                "requirements": {"capabilities": ["analysis", "report"]},
                "context": {"dataset": "s3://bucket/sales_q1.csv"},
                "metadata": {"project": "q1-retrospective"}
            })),
            response_example: json!({"message": "Create task endpoint"}),
            notes: "当前为占位处理器；数据模型已定义。",
        },
        UiEndpoint {
            group: "Task",
            method: "GET",
            path: "/tasks",
            description: "任务列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get tasks endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Task",
            method: "GET",
            path: "/tasks/{task_id}",
            description: "任务详情",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get task endpoint"}),
            notes: "将 {task_id} 替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Task",
            method: "PUT",
            path: "/tasks/{task_id}",
            description: "更新任务",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "title": "分析 Q1 销售数据（修订）",
                "description": "补充区域维度结论",
                "status": "in_progress",
                "priority": "high",
                "progress": 45
            })),
            response_example: json!({"message": "Update task endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Task",
            method: "DELETE",
            path: "/tasks/{task_id}",
            description: "删除任务",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Delete task endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Task",
            method: "PUT",
            path: "/tasks/{task_id}/status",
            description: "更新任务状态",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "status": "in_progress",
                "progress": 60,
                "current_step": "生成图表"
            })),
            response_example: json!({"message": "Update task status endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Task",
            method: "POST",
            path: "/tasks/{task_id}/decompose",
            description: "任务分解",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "strategy": "Hierarchical",
                "max_depth": 3,
                "constraints": {"max_subtasks": 8}
            })),
            response_example: json!({"message": "Decompose task endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Task",
            method: "GET",
            path: "/tasks/{task_id}/subtasks",
            description: "查询子任务",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get subtasks endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Agent",
            method: "GET",
            path: "/agents",
            description: "智能体列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get agents endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Agent",
            method: "POST",
            path: "/agents",
            description: "注册智能体",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "name": "analysis-agent-1",
                "description": "数据分析智能体",
                "capabilities": [
                    {
                        "name": "analysis",
                        "description": "data analysis capability",
                        "version": "1.0",
                        "parameters": {}
                    }
                ],
                "endpoints": {
                    "task_execution": "http://127.0.0.1:9001/run",
                    "health_check": "http://127.0.0.1:9001/health",
                    "status_update": null
                },
                "limits": {
                    "max_concurrent_tasks": 4,
                    "max_execution_time": 600,
                    "max_memory_usage": null,
                    "rate_limit_per_minute": 60
                },
                "metadata": {"owner": "platform-team"}
            })),
            response_example: json!({"message": "Register agent endpoint"}),
            notes: "已接入 agent_service。",
        },
        UiEndpoint {
            group: "Agent",
            method: "GET",
            path: "/agents/{agent_id}",
            description: "智能体详情",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get agent endpoint"}),
            notes: "将 {agent_id} 替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Agent",
            method: "POST",
            path: "/agents/{agent_id}/heartbeat",
            description: "心跳更新",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "current_load": 1,
                "current_task_id": null,
                "metrics": {"latency_ms": 120}
            })),
            response_example: json!({"message": "Update heartbeat endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Agent",
            method: "PUT",
            path: "/agents/{agent_id}/status",
            description: "更新智能体状态",
            auth: true,
            implemented: true,
            request_example: Some(json!({"status": "online"})),
            response_example: json!({"message": "Update agent status endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Agent",
            method: "POST",
            path: "/agents/{agent_id}/assign-task",
            description: "分配任务给智能体",
            auth: true,
            implemented: true,
            request_example: Some(json!({"task_id": "00000000-0000-0000-0000-000000000002"})),
            response_example: json!({"message": "Assign task to agent endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Agent",
            method: "POST",
            path: "/agents/{agent_id}/complete-task",
            description: "完成任务并释放智能体",
            auth: true,
            implemented: true,
            request_example: Some(json!({"task_id": "00000000-0000-0000-0000-000000000002"})),
            response_example: json!({"message": "Complete task and release agent endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Agent",
            method: "GET",
            path: "/agents/stats",
            description: "智能体统计",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get agent stats endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "POST",
            path: "/workspaces",
            description: "创建工作空间",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "name": "Q1 增长分析",
                "description": "市场增长复盘",
                "is_public": false,
                "context": {"department": "growth"},
                "metadata": {"owner_team": "growth-data"}
            })),
            response_example: json!({"message": "Create workspace endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "GET",
            path: "/workspaces",
            description: "工作空间列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workspaces endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "GET",
            path: "/workspaces/{workspace_id}",
            description: "工作空间详情",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workspace endpoint"}),
            notes: "将 {workspace_id} 替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "PUT",
            path: "/workspaces/{workspace_id}",
            description: "更新工作空间",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "name": "Q1 增长分析（修订）",
                "description": "新增同比分析",
                "is_public": false
            })),
            response_example: json!({"message": "Update workspace endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "DELETE",
            path: "/workspaces/{workspace_id}",
            description: "删除工作空间",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Delete workspace endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "GET",
            path: "/workspaces/{workspace_id}/permissions",
            description: "权限列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workspace permissions endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "POST",
            path: "/workspaces/{workspace_id}/permissions",
            description: "授予权限",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "user_id": "00000000-0000-0000-0000-000000000003",
                "permission_level": "write",
                "expires_at": null
            })),
            response_example: json!({"message": "Grant permission endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "DELETE",
            path: "/workspaces/{workspace_id}/permissions/{permission_id}",
            description: "撤销权限",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Revoke permission endpoint"}),
            notes: "将路径参数替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "GET",
            path: "/workspaces/{workspace_id}/documents",
            description: "文档列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workspace documents endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "GET",
            path: "/workspaces/{workspace_id}/tools",
            description: "工具列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workspace tools endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workspace",
            method: "GET",
            path: "/workspaces/{workspace_id}/stats",
            description: "工作空间统计",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workspace stats endpoint"}),
            notes: "当前为占位处理器。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "GET",
            path: "/workflows",
            description: "工作流列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workflows endpoint"}),
            notes: "支持可选 query 参数 workspace_id。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "POST",
            path: "/workflows",
            description: "创建工作流",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "name": "月度数据流水线",
                "description": "从采集到报告自动执行",
                "workspace_id": "00000000-0000-0000-0000-000000000000",
                "definition": {
                    "nodes": [{"id": "collect"}, {"id": "analyze"}, {"id": "report"}],
                    "edges": [["collect", "analyze"], ["analyze", "report"]]
                }
            })),
            response_example: json!({"message": "Create workflow endpoint"}),
            notes: "将 workspace_id 替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "GET",
            path: "/workflows/{workflow_id}",
            description: "工作流详情",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workflow endpoint"}),
            notes: "将 {workflow_id} 替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "DELETE",
            path: "/workflows/{workflow_id}",
            description: "删除工作流",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Delete workflow endpoint"}),
            notes: "创建者或工作空间 owner 可删除。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "POST",
            path: "/workflows/{workflow_id}/execute",
            description: "执行工作流",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "input": {"month": "2026-02", "region": "APAC"},
                "options": {"priority": "high"}
            })),
            response_example: json!({"message": "Execute workflow endpoint"}),
            notes: "会创建任务并尝试调度到在线智能体。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "GET",
            path: "/workflows/{workflow_id}/executions",
            description: "工作流执行列表",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workflow executions endpoint"}),
            notes: "支持 query: limit, offset。",
        },
        UiEndpoint {
            group: "Workflow",
            method: "GET",
            path: "/workflows/{workflow_id}/executions/{execution_id}",
            description: "工作流执行详情",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get workflow execution endpoint"}),
            notes: "将路径参数替换为实际 UUID。",
        },
        UiEndpoint {
            group: "Message",
            method: "POST",
            path: "/messages",
            description: "发送消息",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "target_type": "user",
                "target_id": "00000000-0000-0000-0000-000000000000",
                "message_type": "notice",
                "content": "hello"
            })),
            response_example: json!({"message": "Send message endpoint"}),
            notes: "target_type 支持 agent/task/user/system。",
        },
        UiEndpoint {
            group: "Message",
            method: "GET",
            path: "/messages/user",
            description: "我的用户消息",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get user messages endpoint"}),
            notes: "支持 query: limit, offset。",
        },
        UiEndpoint {
            group: "Message",
            method: "GET",
            path: "/messages/user/unread-count",
            description: "我的未读消息数",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"data": {"unread_count": 3}}),
            notes: "返回当前用户未读 user 消息数。",
        },
        UiEndpoint {
            group: "Message",
            method: "GET",
            path: "/messages/agent/{agent_id}",
            description: "智能体消息",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get agent messages endpoint"}),
            notes: "支持 query: limit, offset。",
        },
        UiEndpoint {
            group: "Message",
            method: "GET",
            path: "/messages/task/{task_id}",
            description: "任务消息",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Get task messages endpoint"}),
            notes: "支持 query: limit, offset。",
        },
        UiEndpoint {
            group: "Message",
            method: "POST",
            path: "/messages/{message_type}/{message_id}/read",
            description: "标记消息已读",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Mark message read endpoint"}),
            notes: "message_type 支持 agent/user。",
        },
        UiEndpoint {
            group: "Message",
            method: "DELETE",
            path: "/messages/{message_type}/{message_id}",
            description: "删除消息",
            auth: true,
            implemented: true,
            request_example: None,
            response_example: json!({"message": "Delete message endpoint"}),
            notes: "message_type 支持 agent/user/task。",
        },
        UiEndpoint {
            group: "Message",
            method: "POST",
            path: "/messages/{message_type}/read-batch",
            description: "批量标记已读",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "message_ids": ["00000000-0000-0000-0000-000000000000"]
            })),
            response_example: json!({"data": {"affected_count": 1}}),
            notes: "user 类型会校验消息归属。",
        },
        UiEndpoint {
            group: "Message",
            method: "POST",
            path: "/messages/{message_type}/delete-batch",
            description: "批量删除消息",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "message_ids": ["00000000-0000-0000-0000-000000000000"]
            })),
            response_example: json!({"data": {"affected_count": 1}}),
            notes: "user 类型会校验消息归属。",
        },
        UiEndpoint {
            group: "Message",
            method: "POST",
            path: "/messages/broadcast",
            description: "发送系统广播",
            auth: true,
            implemented: true,
            request_example: Some(json!({
                "message_type": "system_notice",
                "content": "maintenance at 22:00"
            })),
            response_example: json!({"message": "Send broadcast endpoint"}),
            notes: "发送到 Redis 频道 system:broadcast。",
        },
    ]
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
struct AgentListQuery {
    capabilities: Option<String>,
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
    let auth = state.auth_service.refresh_token(&request.refresh_token).await?;
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
    let task = state.task_service.update_task(task_id, request, user_id).await?;
    Ok(success_response(json!(task.to_response(vec![])), "任务更新成功"))
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
    Ok(success_response(json!(task.to_response(vec![])), "任务状态更新成功"))
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
    Query(query): Query<AgentListQuery>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let caps = query.capabilities.map(|v| {
        v.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
    });
    let agents = state.agent_service.get_available_agents(caps).await?;
    let data: Vec<_> = agents.into_iter().map(|a| a.to_response()).collect();
    Ok(success_response(json!(data), "智能体列表获取成功"))
}

async fn register_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterAgentRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let agent = state.agent_service.register_agent(request).await?;
    Ok(success_response(json!(agent.to_response()), "智能体注册成功"))
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
    Ok(success_response(json!(agent.to_response()), "智能体详情获取成功"))
}

async fn update_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<AgentHeartbeatRequest>,
) -> Result<Json<Value>, AppError> {
    let _ = extract_user_id(&headers)?;
    let health = state.agent_service.update_heartbeat(agent_id, request).await?;
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
    Ok(success_response(json!(agent.to_response()), "智能体状态更新成功"))
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
    Ok(success_response(json!(permission.to_response()), "权限授予成功"))
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
    Ok(success_response(json!(workflow.to_response()), "工作流创建成功"))
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
    Ok(success_response(json!(workflow.to_response()), "工作流详情获取成功"))
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
        return Err(AppError::ValidationError("工作流已禁用，无法执行".to_string()));
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
        context: json!({
            "workflow_id": workflow_id,
            "workflow_execution_id": execution.id,
            "input": input,
            "options": options
        }),
        metadata: Some(json!({
            "source": "workflow_execution"
        })),
    };

    let task = match state.task_service.create_task(create_task_req, user_id).await {
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
    Ok(success_response(json!(execution.to_response()), "工作流执行详情获取成功"))
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
                return Err(AppError::PermissionDenied("没有访问该任务的权限".to_string()));
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
                return Err(AppError::PermissionDenied("只能给自己发送用户消息".to_string()));
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
                .send_system_broadcast(
                    &request.message_type,
                    &request.content,
                    request.metadata,
                )
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
    Ok(success_response(json!({ "unread_count": unread_count }), "未读消息数获取成功"))
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
        return Err(AppError::PermissionDenied("没有访问该任务的权限".to_string()));
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
        return Err(AppError::ValidationError("message_ids 不能为空".to_string()));
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
        return Err(AppError::ValidationError("message_ids 不能为空".to_string()));
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
