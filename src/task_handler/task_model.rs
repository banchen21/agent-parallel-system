use serde::{Deserialize, Serialize};
use uuid::Uuid;

use serde_json::Value;

/// 消息分类响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageClassificationResponse {
    /// 是否为任务
    #[serde(rename = "is_task")]
    pub is_task: bool,

    /// 分类原因
    #[serde(rename = "reason")]
    pub reason: Option<String>,

    /// 任务列表（可能为null）
    #[serde(rename = "tasks")]
    pub tasks: Option<Vec<TaskItem>>,
}

/// 任务项结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    /// 任务ID
    #[serde(rename = "id")]
    pub id: Option<String>,

    /// 任务名称
    #[serde(rename = "name")]
    pub name: Option<String>,

    /// 任务描述
    #[serde(rename = "description")]
    pub description: Option<String>,

    /// 任务优先级
    #[serde(rename = "priority")]
    pub priority: Option<TaskPriority>,

    /// 任务状态
    #[serde(rename = "status")]
    pub status: Option<TaskStatus>,

    /// 任务截止时间
    #[serde(rename = "due_date")]
    pub due_date: Option<String>,
}

/// 任务优先级枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    /// 低
    Low,
    /// 中
    Medium,
    /// 高
    High,
    /// 紧急
    Critical,
}

/// 任务状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 任务已注册并质押奖励，等待接取
    Published,
    /// 已被一个或多个 Agent 认领，处于准备或执行阶段
    Accepted,
    /// Agent 正在实际执行任务
    Executing,
    /// Agent 已提交结果，等待审阅
    Submitted,
    /// 审阅者正在评估结果
    Reviewing,
    /// 审阅通过，奖励已分配
    CompletedSuccess,
    /// 审阅未通过或超时，任务关闭，可能触发惩罚
    CompletedFailure,
    /// 任务发布者在接取前取消任务
    Cancelled,
}

impl MessageClassificationResponse {
    /// 创建新的消息分类响应
    pub fn new(
        is_task: bool,
        confidence: f32,
        content: String,
        reason: String,
        tasks: Option<Vec<TaskItem>>,
    ) -> Self {
        Self {
            is_task,
            reason: Some(reason),
            tasks,
        }
    }

    /// 获取任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.as_ref().map(|t| t.len()).unwrap_or(0)
    }

    /// 是否有任务
    pub fn has_tasks(&self) -> bool {
        self.tasks.is_some() && !self.tasks.as_ref().unwrap().is_empty()
    }

    /// 转换为JSON字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从JSON字符串解析
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

impl Default for MessageClassificationResponse {
    fn default() -> Self {
        Self {
            is_task: false,
            reason: None,
            tasks: None,
        }
    }
}

/// 意图识别响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentClassificationResponse {
    /// 是否为任务
    #[serde(rename = "is_task")]
    pub is_task: bool,

    /// 置信度分数 (0.0 - 1.0)
    #[serde(rename = "confidence")]
    pub confidence: f32,

    /// 不论是否为任务均正常回复内容
    #[serde(rename = "content")]
    pub content: String,

    /// 总任务标题/原因
    #[serde(rename = "reason")]
    pub reason: String,

    /// 任务列表，如果不是任务则为null
    #[serde(rename = "tasks")]
    pub tasks: Option<Vec<TaskDetail>>,
}

/// 任务详情结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    /// 唯一标识符
    #[serde(rename = "task_id")]
    pub task_id: String,

    /// 任务标题
    #[serde(rename = "task")]
    pub task: String,

    /// 任务详细描述
    #[serde(rename = "task_description")]
    pub task_description: String,
}

impl IntentClassificationResponse {
    /// 创建新的意图识别响应
    pub fn new(
        is_task: bool,
        confidence: f32,
        content: String,
        reason: String,
        tasks: Option<Vec<TaskDetail>>,
    ) -> Self {
        Self {
            is_task,
            confidence,
            content,
            reason,
            tasks,
        }
    }

    /// 检查是否为有效分类（置信度阈值检查）
    pub fn is_valid(&self, confidence_threshold: f32) -> bool {
        self.confidence >= confidence_threshold
    }

    /// 获取任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.as_ref().map(|t| t.len()).unwrap_or(0)
    }

    /// 是否有任务
    pub fn has_tasks(&self) -> bool {
        self.tasks.is_some() && !self.tasks.as_ref().unwrap().is_empty()
    }

    /// 从JSON字符串解析
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// 转换为JSON字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl TaskDetail {
    /// 创建新的任务详情
    pub fn new(
        task_id: impl Into<String>,
        task: impl Into<String>,
        task_description: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            task: task.into(),
            task_description: task_description.into(),
        }
    }

    /// 生成UUID作为任务ID
    pub fn with_uuid(task: impl Into<String>, task_description: impl Into<String>) -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            task: task.into(),
            task_description: task_description.into(),
        }
    }
}

// 如果你需要在Actor中使用
#[derive(Debug, Clone, Serialize, Deserialize, actix::Message)]
#[rtype(result = "Result<IntentClassificationResponse, anyhow::Error>")]
pub struct ClassifyMessageIntent {
    pub message: String,
    pub personality_setting: Option<String>,
    pub memory_content: Option<String>,
}

impl Default for IntentClassificationResponse {
    fn default() -> Self {
        Self {
            is_task: false,
            confidence: 0.0,
            content: String::new(),
            reason: String::new(),
            tasks: None,
        }
    }
}
