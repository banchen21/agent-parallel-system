use actix::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use tracing::{debug, error, info};

use crate::utils::workspace_path::workspace_dir;

use crate::{
    agsnets::actor_agents::{
        AgentManageActor, AgentStatus, ExecuteAgentTask, UpdateAgentStatus,
    },
    chat::actor_messages::{ChannelManagerActor, SaveMessage},
    chat::model::{MessageContent, UserMessage},
    chat::openai_actor::ChatAgentError,
    mcp::mcp_actor::McpManagerActor,
    task_handler::task_model::{TaskItem, TaskPriority, TaskStatus},
    task_handler::task_notify_queue::{EnqueueTaskNotify, TaskNotifyQueueActor},
    workspace::model::AgentId,
};

/// 任务 ID 类型（编排器内部使用的唯一键）
pub type TaskId = uuid::Uuid;

#[derive(Debug, Clone)]
struct TaskRecord {
    id: TaskId,
    task_key: String,
    name: String,
    description: String,
    priority: Option<TaskPriority>,
    status: TaskStatus,
    due_date: Option<String>,
    depends_on: Vec<String>,
    required_mcp: Vec<String>,
    mcp_execution_started: bool,
    assigned_agent_id: Option<AgentId>,
    assigned_agent_name: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub task_key: String,
    pub name: String,
    pub description: String,
    pub priority: Option<TaskPriority>,
    pub status: TaskStatus,
    pub status_label: String,
    pub status_group: String,
    pub due_date: Option<String>,
    pub depends_on: Vec<String>,
    pub required_mcp: Vec<String>,
    pub assigned_agent_id: Option<String>,
    pub assigned_agent_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskInput {
    pub task_key: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub priority: Option<TaskPriority>,
    pub due_date: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub required_mcp: Option<Vec<String>>,
}

fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Published => "等待中",
        TaskStatus::Accepted => "已接取",
        TaskStatus::Executing => "执行中",
        TaskStatus::Submitted => "已提交",
        TaskStatus::Reviewing => "审核中",
        TaskStatus::CompletedSuccess => "已完成",
        TaskStatus::CompletedFailure => "失败",
        TaskStatus::Cancelled => "已取消",
    }
}

fn status_group(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Published => "pending",
        TaskStatus::Accepted
        | TaskStatus::Executing
        | TaskStatus::Submitted
        | TaskStatus::Reviewing => "running",
        TaskStatus::CompletedSuccess => "completed",
        TaskStatus::CompletedFailure | TaskStatus::Cancelled => "failed",
    }
}

fn is_active_status(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Accepted | TaskStatus::Executing | TaskStatus::Submitted | TaskStatus::Reviewing
    )
}

fn priority_to_db(priority: &Option<TaskPriority>) -> Option<&'static str> {
    match priority {
        Some(TaskPriority::Low) => Some("low"),
        Some(TaskPriority::Medium) => Some("medium"),
        Some(TaskPriority::High) => Some("high"),
        Some(TaskPriority::Critical) => Some("critical"),
        None => None,
    }
}

fn priority_from_db(value: Option<&str>) -> Option<TaskPriority> {
    match value {
        Some("low") => Some(TaskPriority::Low),
        Some("medium") => Some(TaskPriority::Medium),
        Some("high") => Some(TaskPriority::High),
        Some("critical") => Some(TaskPriority::Critical),
        _ => None,
    }
}

fn task_status_to_db(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Published => "published",
        TaskStatus::Accepted => "accepted",
        TaskStatus::Executing => "executing",
        TaskStatus::Submitted => "submitted",
        TaskStatus::Reviewing => "reviewing",
        TaskStatus::CompletedSuccess => "completed_success",
        TaskStatus::CompletedFailure => "completed_failure",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn task_status_from_db(status: &str) -> TaskStatus {
    match status {
        "accepted" => TaskStatus::Accepted,
        "executing" => TaskStatus::Executing,
        "submitted" => TaskStatus::Submitted,
        "reviewing" => TaskStatus::Reviewing,
        "completed_success" => TaskStatus::CompletedSuccess,
        "completed_failure" => TaskStatus::CompletedFailure,
        "cancelled" => TaskStatus::Cancelled,
        _ => TaskStatus::Published,
    }
}

impl TaskRecord {
    fn from_task_item(task_id: TaskId, task: TaskItem) -> Self {
        let now = Utc::now();
        let task_key = task
            .task_key
            .clone()
            .or(task.id.clone())
            .unwrap_or_else(|| task_id.to_string());
        Self {
            id: task_id,
            task_key,
            name: task.name.unwrap_or_else(|| "未命名任务".to_string()),
            description: task.description.unwrap_or_default(),
            priority: task.priority,
            status: task.status.unwrap_or(TaskStatus::Published),
            due_date: task.due_date,
            depends_on: task.depends_on.unwrap_or_default(),
            required_mcp: task.required_mcp.unwrap_or_default(),
            mcp_execution_started: false,
            assigned_agent_id: None,
            assigned_agent_name: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn from_row(row: sqlx::postgres::PgRow) -> Self {
        let id: uuid::Uuid = row.get("id");
        Self {
            id,
            task_key: id.to_string(),
            name: row.get("name"),
            description: row.get("description"),
            priority: priority_from_db(row.get::<Option<&str>, _>("priority")),
            status: task_status_from_db(row.get::<&str, _>("status")),
            due_date: row.get("due_date"),
            depends_on: Vec::new(),
            required_mcp: Vec::new(),
            mcp_execution_started: false,
            assigned_agent_id: row.get("assigned_agent_id"),
            assigned_agent_name: row.get("assigned_agent_name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn to_view(&self) -> TaskView {
        TaskView {
            id: self.id.to_string(),
            task_key: self.task_key.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            priority: self.priority.clone(),
            status: self.status.clone(),
            status_label: status_label(&self.status).to_string(),
            status_group: status_group(&self.status).to_string(),
            due_date: self.due_date.clone(),
            depends_on: self.depends_on.clone(),
            required_mcp: self.required_mcp.clone(),
            assigned_agent_id: self.assigned_agent_id.map(|id| id.to_string()),
            assigned_agent_name: self.assigned_agent_name.clone(),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.updated_at.to_rfc3339(),
        }
    }
}

pub struct DagOrchestrator {
    pool: PgPool,
    /// 任务表：task_id -> 任务项
    tasks: HashMap<TaskId, TaskRecord>,
    /// 已注册的 Agent：agent_id -> 显示名
    agents: HashMap<AgentId, String>,
    /// Agent 可用 MCP 列表
    agent_mcps: HashMap<AgentId, Vec<String>>,
    /// Agent 所属工作区：agent_id -> workspace_name
    agent_workspaces: HashMap<AgentId, String>,
    /// DAG 节点索引：task_key -> task_id
    task_key_index: HashMap<String, TaskId>,
    /// 任务接取记录：task_id -> 接取的 agent_id
    task_assignments: HashMap<TaskId, AgentId>,
    /// Agent 管理器，用于在任务状态变化时同步智能体状态
    agent_manager: Option<Addr<AgentManageActor>>,
    /// MCP 管理器，用于任务执行阶段触发 MCP 工具
    mcp_manager: Option<Addr<McpManagerActor>>,
    /// 消息通道，用于向用户写入任务进度
    channel_manager: Option<Addr<ChannelManagerActor>>,
    /// 任务通知队列，用于异步广播任务完成通知
    task_notify_queue: Option<Addr<TaskNotifyQueueActor>>,
}

#[derive(Debug)]
pub enum ClaimTaskError {
    TaskNotFound,
    TaskNotPublished,
    AgentNotRegistered,
    TaskBlockedByDependencies,
    AgentMcpNotSatisfied,
}

impl std::fmt::Display for ClaimTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaimTaskError::TaskNotFound => write!(f, "任务不存在"),
            ClaimTaskError::TaskNotPublished => write!(f, "任务未处于待接取状态"),
            ClaimTaskError::AgentNotRegistered => write!(f, "Agent 未注册"),
            ClaimTaskError::TaskBlockedByDependencies => write!(f, "任务依赖尚未完成"),
            ClaimTaskError::AgentMcpNotSatisfied => write!(f, "Agent 缺少任务所需 MCP 能力"),
        }
    }
}

impl std::error::Error for ClaimTaskError {}

impl DagOrchestrator {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            tasks: HashMap::new(),
            agents: HashMap::new(),
            agent_mcps: HashMap::new(),
            agent_workspaces: HashMap::new(),
            task_key_index: HashMap::new(),
            task_assignments: HashMap::new(),
            agent_manager: None,
            mcp_manager: None,
            channel_manager: None,
            task_notify_queue: None,
        }
    }

    fn send_progress_message(&self, text: String) {
        if let Some(channel_manager) = &self.channel_manager {
            let channel_manager = channel_manager.clone();
            actix::spawn(async move {
                let _ = channel_manager
                    .send(SaveMessage {
                        message: UserMessage {
                            sender: "任务系统".to_string(),
                            source_ip: "task_system".to_string(),
                            device_type: "task".to_string(),
                            content: MessageContent::Text(text),
                            created_at: Utc::now(),
                        },
                    })
                    .await;
            });
        }
    }

    fn enqueue_task_notify_message(&self, text: String) {
        if let Some(task_notify_queue) = &self.task_notify_queue {
            task_notify_queue.do_send(EnqueueTaskNotify {
                content: text,
                created_at: Utc::now(),
            });
        }
    }

    /// 追加写入任务记忆日志到对应工作区 memory.log
    fn write_task_memory_log(&self, workspace_name: &str, task: &TaskRecord) {
        let ws_dir = workspace_dir(workspace_name);
        // 确保工作区目录存在（若未初始化）
        if let Err(e) = std::fs::create_dir_all(&ws_dir) {
            error!(workspace = %workspace_name, "创建工作区目录失败: {}", e);
            return;
        }
        let log_path = ws_dir.join("memory.log");
        let entry = format!(
            "[{}] 任务完成 | 任务: {} | 智能体: {}\n  任务ID: {}\n  描述: {}\n---\n",
            task.updated_at.format("%Y-%m-%dT%H:%M:%SZ"),
            task.name,
            task.assigned_agent_name.as_deref().unwrap_or("未知"),
            task.id,
            task.description,
        );
        if let Err(e) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut f| f.write_all(entry.as_bytes()))
        {
            error!(workspace = %workspace_name, task_id = %task.id, "写入 memory.log 失败: {}", e);
        } else {
            info!(workspace = %workspace_name, task_id = %task.id, "任务记忆已写入 memory.log");
        }
    }

    fn sync_agent_status(&self, agent_id: AgentId, status: AgentStatus) {
        if let Some(agent_manager) = &self.agent_manager {
            agent_manager.do_send(UpdateAgentStatus { agent_id, status });
        }
    }

    fn persist_task_snapshot(&self, task: &TaskRecord) {
        let pool = self.pool.clone();
        let task = task.clone();
        actix::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO tasks (
                    id, name, description, priority, status, due_date,
                    assigned_agent_id, assigned_agent_name, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (id) DO UPDATE SET
                    name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    priority = EXCLUDED.priority,
                    status = EXCLUDED.status,
                    due_date = EXCLUDED.due_date,
                    assigned_agent_id = EXCLUDED.assigned_agent_id,
                    assigned_agent_name = EXCLUDED.assigned_agent_name,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(task.id)
            .bind(&task.name)
            .bind(&task.description)
            .bind(priority_to_db(&task.priority))
            .bind(task_status_to_db(&task.status))
            .bind(&task.due_date)
            .bind(task.assigned_agent_id)
            .bind(&task.assigned_agent_name)
            .bind(task.created_at)
            .bind(task.updated_at)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                error!(task_id = %task.id, "任务持久化失败: {}", e);
            }
        });
    }

    fn is_agent_available(&self, agent_id: AgentId) -> bool {
        !self.task_assignments.iter().any(|(task_id, assigned_agent_id)| {
            if *assigned_agent_id != agent_id {
                return false;
            }
            self.tasks
                .get(task_id)
                .map(|task| is_active_status(&task.status))
                .unwrap_or(false)
        })
    }

    fn is_task_dependencies_satisfied(&self, task: &TaskRecord) -> bool {
        if task.depends_on.is_empty() {
            return true;
        }

        task.depends_on.iter().all(|dep_key| {
            self.task_key_index
                .get(dep_key)
                .and_then(|dep_task_id| self.tasks.get(dep_task_id))
                .map(|dep_task| dep_task.status == TaskStatus::CompletedSuccess)
                .unwrap_or(false)
        })
    }

    fn has_required_mcp(&self, agent_id: AgentId, task: &TaskRecord) -> bool {
        if task.required_mcp.is_empty() {
            return true;
        }

        let Some(agent_mcps) = self.agent_mcps.get(&agent_id) else {
            return false;
        };

        task.required_mcp
            .iter()
            .all(|required| agent_mcps.iter().any(|owned| owned == required))
    }

    fn effective_mcp_for_task(&self, task: &TaskRecord) -> Vec<String> {
        if !task.required_mcp.is_empty() {
            return task.required_mcp.clone();
        }

        task.assigned_agent_id
            .and_then(|agent_id| self.agent_mcps.get(&agent_id).cloned())
            .unwrap_or_default()
    }

    /// 尝试将一条 Published 任务分配给一名已注册 Agent（用于内部轮询接取）
    fn try_claim_one_published(&mut self) {
        let mut task_to_claim: Option<TaskId> = None;
        let mut agent_to_assign: Option<AgentId> = None;

        for (task_id, task) in &self.tasks {
            if task.status != TaskStatus::Published {
                continue;
            }
            if self.task_assignments.contains_key(task_id) {
                continue;
            }
            if !self.is_task_dependencies_satisfied(task) {
                continue;
            }
            task_to_claim = Some(*task_id);
            break;
        }

        if let Some(task_id) = task_to_claim {
            for agent_id in self.agents.keys() {
                let can_assign = self
                    .tasks
                    .get(&task_id)
                    .map(|task| {
                        self.is_agent_available(*agent_id)
                            && self.has_required_mcp(*agent_id, task)
                    })
                    .unwrap_or(false);

                if can_assign {
                    agent_to_assign = Some(*agent_id);
                    break;
                }
            }
        }

        if let (Some(tid), Some(aid)) = (task_to_claim, agent_to_assign) {
            let agent_name = self
                .agents
                .get(&aid)
                .cloned()
                .unwrap_or_else(|| "?".to_string());

            let snapshot = if let Some(task) = self.tasks.get_mut(&tid) {
                task.status = TaskStatus::Accepted;
                task.assigned_agent_id = Some(aid);
                task.assigned_agent_name = Some(agent_name.clone());
                task.updated_at = Utc::now();
                Some(task.clone())
            } else {
                None
            };

            if let Some(snapshot) = snapshot {
                self.task_assignments.insert(tid, aid);
                self.sync_agent_status(aid, AgentStatus::Working);
                self.persist_task_snapshot(&snapshot);
                info!(task_id = %tid, agent_id = %aid, agent_name = %agent_name, "任务已接取");
            }
        }
    }

    fn progress_active_tasks(&mut self, ctx: &mut Context<Self>) {
        let now = Utc::now();
        let agent_mcps_snapshot = self.agent_mcps.clone();
        let mut snapshots = Vec::new();
        let mut finished_task_ids = Vec::new();
        let mut complete_agents = Vec::new();
        let mut mcp_jobs: Vec<(TaskId, AgentId, String, Vec<String>)> = Vec::new();
        let mut progress_texts: Vec<String> = Vec::new();
        let mut notify_texts: Vec<String> = Vec::new();
        // (workspace_name, completed_task) 用于写 memory.log
        let mut memory_entries: Vec<(String, TaskRecord)> = Vec::new();

        for (task_id, task) in self.tasks.iter_mut() {
            match task.status {
                TaskStatus::Accepted => {
                    if (now - task.updated_at).num_seconds() >= 2 {
                        task.status = TaskStatus::Executing;
                        task.updated_at = now;
                        progress_texts.push(format!(
                            "任务进度：{} 已开始执行（执行智能体：{}）",
                            task.name,
                            task.assigned_agent_name.as_deref().unwrap_or("未知")
                        ));
                        snapshots.push(task.clone());
                        let effective_mcp = if !task.required_mcp.is_empty() {
                            task.required_mcp.clone()
                        } else {
                            task.assigned_agent_id
                                .and_then(|agent_id| agent_mcps_snapshot.get(&agent_id).cloned())
                                .unwrap_or_default()
                        };
                        if !effective_mcp.is_empty() && !task.mcp_execution_started {
                            task.mcp_execution_started = true;
                            snapshots.push(task.clone());
                            if let Some(agent_id) = task.assigned_agent_id {
                                mcp_jobs.push((
                                    *task_id,
                                    agent_id,
                                    task.description.clone(),
                                    effective_mcp,
                                ));
                            }
                        }
                        info!(task_id = %task_id, "任务进入执行中");
                    }
                }
                TaskStatus::Executing => {
                    let has_real_mcp_job = if !task.required_mcp.is_empty() {
                        true
                    } else {
                        task.assigned_agent_id
                            .and_then(|agent_id| agent_mcps_snapshot.get(&agent_id))
                            .map(|list| !list.is_empty())
                            .unwrap_or(false)
                    };

                    if !has_real_mcp_job && (now - task.updated_at).num_seconds() >= 3 {
                        task.status = TaskStatus::Submitted;
                        task.updated_at = now;
                        snapshots.push(task.clone());
                        info!(task_id = %task_id, "任务已提交结果");
                    }
                }
                TaskStatus::Submitted => {
                    if (now - task.updated_at).num_seconds() >= 2 {
                        task.status = TaskStatus::Reviewing;
                        task.updated_at = now;
                        progress_texts.push(format!(
                            "任务进度：{} 已提交结果，进入审核中",
                            task.name
                        ));
                        snapshots.push(task.clone());
                        info!(task_id = %task_id, "任务进入审核中");
                    }
                }
                TaskStatus::Reviewing => {
                    if (now - task.updated_at).num_seconds() >= 2 {
                        task.status = TaskStatus::CompletedSuccess;
                        task.updated_at = now;
                        progress_texts.push(format!(
                            "任务进度：{} 已完成",
                            task.name
                        ));
                        notify_texts.push(format!(
                            "任务完成：{}（执行智能体：{}）",
                            task.name,
                            task.assigned_agent_name.as_deref().unwrap_or("未知")
                        ));
                        snapshots.push(task.clone());
                        finished_task_ids.push(*task_id);
                        if let Some(agent_id) = task.assigned_agent_id {
                            complete_agents.push(agent_id);
                            if let Some(ws) = self.agent_workspaces.get(&agent_id) {
                                memory_entries.push((ws.clone(), task.clone()));
                            }
                        }
                        info!(task_id = %task_id, "任务已完成");
                    }
                }
                _ => {}
            }
        }

        for task_id in finished_task_ids {
            self.task_assignments.remove(&task_id);
        }

        for snapshot in snapshots {
            self.persist_task_snapshot(&snapshot);
        }

        for text in progress_texts {
            self.send_progress_message(text);
        }

        for text in notify_texts {
            self.enqueue_task_notify_message(text);
        }

        if let Some(agent_manager) = self.agent_manager.clone() {
            let addr = ctx.address();
            for (task_id, agent_id, input, mcp_names) in mcp_jobs {
                let actor = agent_manager.clone();
                let addr = addr.clone();
                actix::spawn(async move {
                    let (success, output) = match actor
                        .send(ExecuteAgentTask {
                            agent_id,
                            task_id: task_id.to_string(),
                            input,
                            mcp_names,
                        })
                        .await
                    {
                        Ok(Ok(result)) => (result.success, result.output),
                        Ok(Err(e)) => (false, format!("agent execution failed: {}", e)),
                        Err(e) => (false, format!("agent mailbox failed: {}", e)),
                    };

                    addr.do_send(ApplyMcpExecutionResult {
                        task_id,
                        success,
                        output,
                    });
                });
            }
        }

        for agent_id in complete_agents {
            self.sync_agent_status(agent_id, AgentStatus::Idle);
        }

        for (workspace_name, task) in memory_entries {
            self.write_task_memory_log(&workspace_name, &task);
        }
    }

    fn insert_task(&mut self, task: TaskItem) -> TaskView {
        let task_id = uuid::Uuid::new_v4();
        let record = TaskRecord::from_task_item(task_id, task);
        self.task_key_index.insert(record.task_key.clone(), task_id);
        let view = record.to_view();
        self.tasks.insert(task_id, record.clone());
        self.persist_task_snapshot(&record);
        debug!(task_id = %task_id, "任务已提交，等待接取");
        view
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ApplyMcpExecutionResult {
    pub task_id: TaskId,
    pub success: bool,
    pub output: String,
}

impl Handler<ApplyMcpExecutionResult> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: ApplyMcpExecutionResult, _ctx: &mut Self::Context) -> Self::Result {
        let mut memory_entry: Option<(String, TaskRecord)> = None;
        let mut agent_to_idle: Option<AgentId> = None;
        let mut should_remove_assignment = false;

        let snapshot_opt = if let Some(task) = self.tasks.get_mut(&msg.task_id) {
            if task.status != TaskStatus::Executing {
                return;
            }

            task.updated_at = Utc::now();
            if msg.success {
                task.status = TaskStatus::Submitted;
                info!(task_id = %msg.task_id, "MCP 执行完成，任务进入已提交");
            } else {
                task.status = TaskStatus::CompletedFailure;
                info!(task_id = %msg.task_id, "MCP 执行失败，任务标记为失败");
            }

            if !msg.success {
                if let Some(agent_id) = task.assigned_agent_id {
                    should_remove_assignment = true;
                    agent_to_idle = Some(agent_id);
                }
            }

            if let Some(agent_id) = task.assigned_agent_id {
                if let Some(ws) = self.agent_workspaces.get(&agent_id) {
                    let mut logged_task = task.clone();
                    logged_task.description = format!(
                        "{}\n\n[MCP 执行日志]\n{}",
                        logged_task.description, msg.output
                    );
                    memory_entry = Some((ws.clone(), logged_task));
                }
            }

            Some(task.clone())
        } else {
            None
        };

        if should_remove_assignment {
            self.task_assignments.remove(&msg.task_id);
        }
        if let Some(agent_id) = agent_to_idle {
            self.sync_agent_status(agent_id, AgentStatus::Idle);
        }

        if !msg.success {
            if let Some(task) = self.tasks.get(&msg.task_id) {
                self.send_progress_message(format!(
                    "任务进度：{} 执行失败，原因：{}",
                    task.name, msg.output
                ));
                self.enqueue_task_notify_message(format!(
                    "任务失败：{}，原因：{}",
                    task.name, msg.output
                ));
            }
        }

        if let Some(snapshot) = snapshot_opt {
            self.persist_task_snapshot(&snapshot);
        }
        if let Some((ws, task)) = memory_entry {
            self.write_task_memory_log(&ws, &task);
        }
    }
}

// ---------- 绑定 Agent 管理器 ----------
#[derive(Message)]
#[rtype(result = "()")]
pub struct BindAgentManager {
    pub agent_manager: Addr<AgentManageActor>,
}

impl Handler<BindAgentManager> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: BindAgentManager, _ctx: &mut Self::Context) -> Self::Result {
        self.agent_manager = Some(msg.agent_manager);
        info!("DagOrchestrator 已绑定 AgentManageActor");

        for task in self.tasks.values() {
            if let Some(agent_id) = task.assigned_agent_id {
                if is_active_status(&task.status) {
                    self.sync_agent_status(agent_id, AgentStatus::Working);
                }
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BindMcpManager {
    pub mcp_manager: Addr<McpManagerActor>,
}

impl Handler<BindMcpManager> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: BindMcpManager, _ctx: &mut Self::Context) -> Self::Result {
        self.mcp_manager = Some(msg.mcp_manager);
        info!("DagOrchestrator 已绑定 McpManagerActor");
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BindChannelManager {
    pub channel_manager: Addr<ChannelManagerActor>,
}

impl Handler<BindChannelManager> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: BindChannelManager, _ctx: &mut Self::Context) -> Self::Result {
        self.channel_manager = Some(msg.channel_manager);
        info!("DagOrchestrator 已绑定 ChannelManagerActor");
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BindTaskNotifyQueue {
    pub task_notify_queue: Addr<TaskNotifyQueueActor>,
}

impl Handler<BindTaskNotifyQueue> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: BindTaskNotifyQueue, _ctx: &mut Self::Context) -> Self::Result {
        self.task_notify_queue = Some(msg.task_notify_queue);
        info!("DagOrchestrator 已绑定 TaskNotifyQueueActor");
    }
}

impl Actor for DagOrchestrator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let pool = self.pool.clone();
        ctx.spawn(
            actix::fut::wrap_future(async move {
                sqlx::query(
                    r#"
                    SELECT id, name, description, priority, status, due_date,
                           assigned_agent_id, assigned_agent_name, created_at, updated_at
                    FROM tasks
                    ORDER BY created_at DESC
                    "#,
                )
                .fetch_all(&pool)
                .await
            })
            .map(|res, actor: &mut Self, _ctx| match res {
                Ok(rows) => {
                    actor.tasks.clear();
                    actor.task_assignments.clear();
                    actor.task_key_index.clear();
                    for row in rows {
                        let task = TaskRecord::from_row(row);
                        actor.task_key_index.insert(task.task_key.clone(), task.id);
                        if let Some(agent_id) = task.assigned_agent_id {
                            if is_active_status(&task.status) {
                                actor.task_assignments.insert(task.id, agent_id);
                            }
                        }
                        actor.tasks.insert(task.id, task);
                    }
                    info!(count = actor.tasks.len(), "任务已从数据库恢复到编排器");
                }
                Err(e) => {
                    error!("恢复任务列表失败: {}", e);
                }
            }),
        );

        ctx.run_interval(std::time::Duration::from_secs(1), |act, ctx| {
            act.try_claim_one_published();
            act.progress_active_tasks(ctx);
        });
    }
}

// ---------- 创建任务 ----------
#[derive(Message)]
#[rtype(result = "Result<TaskView, ChatAgentError>")]
pub struct SubmitTask {
    pub task: TaskItem,
}

impl Handler<SubmitTask> for DagOrchestrator {
    type Result = Result<TaskView, ChatAgentError>;

    fn handle(&mut self, msg: SubmitTask, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.insert_task(msg.task))
    }
}

// ---------- HTTP 创建任务 ----------
#[derive(Message)]
#[rtype(result = "TaskView")]
pub struct CreateTask {
    pub input: CreateTaskInput,
}

impl Handler<CreateTask> for DagOrchestrator {
    type Result = actix::MessageResult<CreateTask>;

    fn handle(&mut self, msg: CreateTask, _ctx: &mut Self::Context) -> Self::Result {
        actix::MessageResult(self.insert_task(TaskItem {
            id: None,
            task_key: msg.input.task_key,
            name: Some(msg.input.name),
            description: Some(msg.input.description),
            priority: msg.input.priority,
            status: Some(TaskStatus::Published),
            due_date: msg.input.due_date,
            depends_on: msg.input.depends_on,
            required_mcp: msg.input.required_mcp,
        }))
    }
}

// ---------- 任务列表 ----------
#[derive(Message)]
#[rtype(result = "Vec<TaskView>")]
pub struct ListTasks;

impl Handler<ListTasks> for DagOrchestrator {
    type Result = Vec<TaskView>;

    fn handle(&mut self, _msg: ListTasks, _ctx: &mut Self::Context) -> Self::Result {
        let mut tasks: Vec<TaskView> = self.tasks.values().map(TaskRecord::to_view).collect();
        tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        tasks
    }
}

// ---------- 单任务详情 ----------
#[derive(Message)]
#[rtype(result = "Option<TaskView>")]
pub struct GetTask {
    pub task_id: TaskId,
}

impl Handler<GetTask> for DagOrchestrator {
    type Result = Option<TaskView>;

    fn handle(&mut self, msg: GetTask, _ctx: &mut Self::Context) -> Self::Result {
        self.tasks.get(&msg.task_id).map(TaskRecord::to_view)
    }
}

// ---------- 注册 Agent（接取任务前必须先注册）----------
#[derive(Message)]
#[rtype(result = "()")]
pub struct RegisterAgent {
    pub agent_id: AgentId,
    pub name: String,
    pub workspace_name: String,
    pub mcp_list: Vec<String>,
}

impl Handler<RegisterAgent> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: RegisterAgent, _ctx: &mut Self::Context) -> Self::Result {
        info!(agent_id = %msg.agent_id, name = %msg.name, workspace = %msg.workspace_name, "Agent 已注册");
        self.agents.insert(msg.agent_id, msg.name);
        self.agent_workspaces.insert(msg.agent_id, msg.workspace_name);
        self.agent_mcps.insert(msg.agent_id, msg.mcp_list);

        for task in self.tasks.values() {
            if task.assigned_agent_id == Some(msg.agent_id) && is_active_status(&task.status) {
                self.sync_agent_status(msg.agent_id, AgentStatus::Working);
            }
        }
    }
}

// ---------- 接取任务（显式由某 Agent 认领）----------
#[derive(Message)]
#[rtype(result = "Result<(), ClaimTaskError>")]
pub struct ClaimTask {
    pub task_id: TaskId,
    pub agent_id: AgentId,
}

impl Handler<ClaimTask> for DagOrchestrator {
    type Result = Result<(), ClaimTaskError>;

    fn handle(&mut self, msg: ClaimTask, _ctx: &mut Self::Context) -> Self::Result {
        if !self.agents.contains_key(&msg.agent_id) {
            return Err(ClaimTaskError::AgentNotRegistered);
        }

        let precheck_task = self
            .tasks
            .get(&msg.task_id)
            .ok_or(ClaimTaskError::TaskNotFound)?;
        if precheck_task.status != TaskStatus::Published {
            return Err(ClaimTaskError::TaskNotPublished);
        }
        if !self.is_task_dependencies_satisfied(precheck_task) {
            return Err(ClaimTaskError::TaskBlockedByDependencies);
        }
        if !self.has_required_mcp(msg.agent_id, precheck_task) {
            return Err(ClaimTaskError::AgentMcpNotSatisfied);
        }

        let agent_name = self
            .agents
            .get(&msg.agent_id)
            .cloned()
            .unwrap_or_else(|| "?".to_string());

        let snapshot = {
            let task = self
                .tasks
                .get_mut(&msg.task_id)
                .ok_or(ClaimTaskError::TaskNotFound)?;
            task.status = TaskStatus::Accepted;
            task.assigned_agent_id = Some(msg.agent_id);
            task.assigned_agent_name = Some(agent_name.clone());
            task.updated_at = Utc::now();
            task.clone()
        };

        self.task_assignments.insert(msg.task_id, msg.agent_id);
        self.sync_agent_status(msg.agent_id, AgentStatus::Working);
        self.persist_task_snapshot(&snapshot);
        info!(
            task_id = %msg.task_id,
            agent_id = %msg.agent_id,
            agent_name = %agent_name,
            "任务已被 Agent 接取"
        );
        Ok(())
    }
}
