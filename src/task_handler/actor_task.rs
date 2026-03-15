use actix::prelude::*;
use std::collections::HashMap;
use tracing::{debug, info};

use crate::{
    chat::openai_actor::ChatAgentError,
    task_handler::task_model::{TaskItem, TaskStatus}, workspace::model::AgentId,
};

/// 任务 ID 类型（编排器内部使用的唯一键）
pub type TaskId = uuid::Uuid;

pub struct DagOrchestrator {
    /// 任务表：task_id -> 任务项
    tasks: HashMap<TaskId, TaskItem>,
    /// 已注册的 Agent：agent_id -> 显示名
    agents: HashMap<AgentId, String>,
    /// 任务接取记录：task_id -> 接取的 agent_id
    task_assignments: HashMap<TaskId, AgentId>,
}

#[derive(Debug)]
pub enum ClaimTaskError {
    TaskNotFound,
    TaskNotPublished,
    AgentNotRegistered,
}

impl std::fmt::Display for ClaimTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaimTaskError::TaskNotFound => write!(f, "任务不存在"),
            ClaimTaskError::TaskNotPublished => write!(f, "任务未处于待接取状态"),
            ClaimTaskError::AgentNotRegistered => write!(f, "Agent 未注册"),
        }
    }
}

impl std::error::Error for ClaimTaskError {}

impl DagOrchestrator {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            agents: HashMap::new(),
            task_assignments: HashMap::new(),
        }
    }

    /// 尝试将一条 Published 任务分配给一名已注册 Agent（用于内部轮询接取）
    fn try_claim_one_published(&mut self) {
        let mut task_to_claim: Option<TaskId> = None;
        let mut agent_to_assign: Option<AgentId> = None;

        for (task_id, task) in &self.tasks {
            if task.status != Some(TaskStatus::Published) {
                continue;
            }
            if self.task_assignments.contains_key(task_id) {
                continue;
            }
            task_to_claim = Some(*task_id);
            break;
        }

        if let Some(_) = task_to_claim {
            for agent_id in self.agents.keys() {
                agent_to_assign = Some(*agent_id);
                break;
            }
        }

        if let (Some(tid), Some(aid)) = (task_to_claim, agent_to_assign) {
            if let Some(task) = self.tasks.get_mut(&tid) {
                task.status = Some(TaskStatus::Accepted);
                self.task_assignments.insert(tid, aid);
                let agent_name = self.agents.get(&aid).map(String::as_str).unwrap_or("?");
                info!(
                    task_id = %tid,
                    agent_id = %aid,
                    agent_name = %agent_name,
                    "任务已接取"
                );
            }
        }
    }
}

impl Actor for DagOrchestrator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // 每秒检查一次：将一条 Published 任务分配给一名已注册 Agent
        ctx.run_interval(std::time::Duration::from_secs(1), |act, _ctx| {
            act.try_claim_one_published();
        });
    }
}

// ---------- 创建任务 ----------
#[derive(Message)]
#[rtype(result = "Result<(), ChatAgentError>")]
pub struct SubmitTask {
    pub task: TaskItem,
}

impl Handler<SubmitTask> for DagOrchestrator {
    type Result = Result<(), ChatAgentError>;

    fn handle(&mut self, msg: SubmitTask, _ctx: &mut Self::Context) -> Self::Result {
        let task_id = uuid::Uuid::new_v4();
        let mut task = msg.task;
        if task.status.is_none() {
            task.status = Some(TaskStatus::Published);
        }
        self.tasks.insert(task_id, task);
        debug!(task_id = %task_id, "任务已提交，等待接取");
        Ok(())
    }
}

// ---------- 注册 Agent（接取任务前必须先注册）----------
#[derive(Message)]
#[rtype(result = "()")]
pub struct RegisterAgent {
    pub agent_id: AgentId,
    pub name: String,
}

impl Handler<RegisterAgent> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: RegisterAgent, _ctx: &mut Self::Context) -> Self::Result {
        info!(agent_id = %msg.agent_id, name = %msg.name, "Agent 已注册");
        self.agents.insert(msg.agent_id, msg.name);
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
        let task = self
            .tasks
            .get_mut(&msg.task_id)
            .ok_or(ClaimTaskError::TaskNotFound)?;
        if task.status != Some(TaskStatus::Published) {
            return Err(ClaimTaskError::TaskNotPublished);
        }
        if !self.agents.contains_key(&msg.agent_id) {
            return Err(ClaimTaskError::AgentNotRegistered);
        }
        task.status = Some(TaskStatus::Accepted);
        self.task_assignments.insert(msg.task_id, msg.agent_id);
        let agent_name = self
            .agents
            .get(&msg.agent_id)
            .map(String::as_str)
            .unwrap_or("?");
        info!(
            task_id = %msg.task_id,
            agent_id = %msg.agent_id,
            agent_name = %agent_name,
            "任务已被 Agent 接取"
        );
        Ok(())
    }
}
