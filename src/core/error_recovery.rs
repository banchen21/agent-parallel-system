use std::collections::HashMap;
use chrono::{Utc};
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::task::{Task, TaskStatus},
};

/// 错误恢复策略
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecoveryStrategy {
    /// 立即重试
    ImmediateRetry,
    /// 指数退避重试
    ExponentialBackoff,
    /// 回滚到上一个检查点
    RollbackToCheckpoint,
    /// 跳过此任务继续执行
    SkipAndContinue,
    /// 停止整个工作流
    StopWorkflow,
}

/// 错误恢复配置
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始重试延迟（秒）
    pub initial_retry_delay: u32,
    /// 最大重试延迟（秒）
    pub max_retry_delay: u32,
    /// 重试策略
    pub strategy: RecoveryStrategy,
    /// 是否启用检查点
    pub enable_checkpoints: bool,
    /// 检查点间隔（秒）
    pub checkpoint_interval: u32,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_retry_delay: 5,
            max_retry_delay: 300, // 5分钟
            strategy: RecoveryStrategy::ExponentialBackoff,
            enable_checkpoints: true,
            checkpoint_interval: 60, // 1分钟
        }
    }
}

/// 任务执行状态快照（检查点）
#[derive(Debug, Clone)]
pub struct TaskCheckpoint {
    pub task_id: Uuid,
    pub timestamp: chrono::DateTime<Utc>,
    pub status: TaskStatus,
    pub progress: i32,
    pub result: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
}

/// 错误恢复管理器
pub struct ErrorRecoveryManager {
    config: RecoveryConfig,
    task_retry_counts: HashMap<Uuid, u32>,
    task_checkpoints: HashMap<Uuid, Vec<TaskCheckpoint>>,
    task_errors: HashMap<Uuid, Vec<String>>,
}

impl ErrorRecoveryManager {
    /// 创建新的错误恢复管理器
    pub fn new(config: RecoveryConfig) -> Self {
        Self {
            config,
            task_retry_counts: HashMap::new(),
            task_checkpoints: HashMap::new(),
            task_errors: HashMap::new(),
        }
    }

    /// 创建默认的错误恢复管理器
    pub fn default() -> Self {
        Self::new(RecoveryConfig::default())
    }

    /// 记录任务错误
    pub fn record_error(&mut self, task_id: Uuid, error: String) {
        self.task_errors
            .entry(task_id)
            .or_insert_with(Vec::new)
            .push(error);
    }

    /// 获取任务错误历史
    pub fn get_task_errors(&self, task_id: Uuid) -> Vec<String> {
        self.task_errors
            .get(&task_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 检查任务是否应该重试
    pub fn should_retry(&mut self, task_id: Uuid) -> bool {
        let retry_count = self.task_retry_counts.entry(task_id).or_insert(0);
        
        if *retry_count >= self.config.max_retries {
            false
        } else {
            *retry_count += 1;
            true
        }
    }

    /// 获取下一次重试的延迟时间（秒）
    pub fn get_retry_delay(&self, task_id: Uuid) -> u32 {
        let retry_count = self.task_retry_counts.get(&task_id).copied().unwrap_or(0);
        
        match self.config.strategy {
            RecoveryStrategy::ImmediateRetry => 0,
            RecoveryStrategy::ExponentialBackoff => {
                let delay = self.config.initial_retry_delay * 2u32.pow(retry_count);
                delay.min(self.config.max_retry_delay)
            }
            RecoveryStrategy::RollbackToCheckpoint => self.config.initial_retry_delay,
            RecoveryStrategy::SkipAndContinue => 0,
            RecoveryStrategy::StopWorkflow => 0,
        }
    }

    /// 创建任务检查点
    pub fn create_checkpoint(&mut self, task: &Task) {
        if !self.config.enable_checkpoints {
            return;
        }

        let checkpoint = TaskCheckpoint {
            task_id: task.id,
            timestamp: Utc::now(),
            status: task.status.clone(),
            progress: task.progress,
            result: task.result.clone(),
            metadata: task.metadata.clone(),
        };

        self.task_checkpoints
            .entry(task.id)
            .or_insert_with(Vec::new)
            .push(checkpoint);

        // 清理旧的检查点，只保留最近的5个
        if let Some(checkpoints) = self.task_checkpoints.get_mut(&task.id) {
            if checkpoints.len() > 5 {
                checkpoints.remove(0);
            }
        }
    }

    /// 获取最新的检查点
    pub fn get_latest_checkpoint(&self, task_id: Uuid) -> Option<&TaskCheckpoint> {
        self.task_checkpoints
            .get(&task_id)
            .and_then(|checkpoints| checkpoints.last())
    }

    /// 回滚到上一个检查点
    pub fn rollback_to_checkpoint(&self, task_id: Uuid) -> Result<TaskCheckpoint, AppError> {
        let checkpoint = self.get_latest_checkpoint(task_id)
            .ok_or_else(|| AppError::NotFound(format!("任务 {} 没有可用的检查点", task_id)))?;

        Ok(checkpoint.clone())
    }

    /// 根据错误类型选择合适的恢复策略
    pub fn select_recovery_strategy(&self, error_type: &str) -> RecoveryStrategy {
        match error_type {
            "timeout" | "network_error" => RecoveryStrategy::ExponentialBackoff,
            "data_corruption" | "validation_error" => RecoveryStrategy::RollbackToCheckpoint,
            "resource_unavailable" => RecoveryStrategy::SkipAndContinue,
            "fatal_error" | "system_error" => RecoveryStrategy::StopWorkflow,
            _ => self.config.strategy,
        }
    }

    /// 处理任务失败
    pub fn handle_task_failure(
        &mut self,
        task_id: Uuid,
        error: String,
        error_type: &str,
    ) -> Result<RecoveryStrategy, AppError> {
        // 记录错误
        self.record_error(task_id, error.clone());

        // 选择恢复策略
        let strategy = self.select_recovery_strategy(error_type);

        // 检查是否应该重试
        let should_retry = match strategy {
            RecoveryStrategy::ImmediateRetry | RecoveryStrategy::ExponentialBackoff => {
                self.should_retry(task_id)
            }
            _ => false,
        };

        if should_retry {
            Ok(strategy)
        } else {
            // 超过最大重试次数，使用备选策略
            match strategy {
                RecoveryStrategy::ImmediateRetry | RecoveryStrategy::ExponentialBackoff => {
                    Ok(RecoveryStrategy::SkipAndContinue)
                }
                _ => Ok(strategy),
            }
        }
    }

    /// 重置任务的重试计数
    pub fn reset_task_retry_count(&mut self, task_id: Uuid) {
        self.task_retry_counts.remove(&task_id);
    }

    /// 清理任务的历史数据
    pub fn cleanup_task_history(&mut self, task_id: Uuid) {
        self.task_retry_counts.remove(&task_id);
        self.task_checkpoints.remove(&task_id);
        self.task_errors.remove(&task_id);
    }

    /// 获取任务统计信息
    pub fn get_task_stats(&self, task_id: Uuid) -> TaskRecoveryStats {
        let retry_count = self.task_retry_counts.get(&task_id).copied().unwrap_or(0);
        let error_count = self.task_errors
            .get(&task_id)
            .map(|errors| errors.len())
            .unwrap_or(0);
        let checkpoint_count = self.task_checkpoints
            .get(&task_id)
            .map(|checkpoints| checkpoints.len())
            .unwrap_or(0);

        TaskRecoveryStats {
            task_id,
            retry_count,
            error_count,
            checkpoint_count,
            last_error: self.task_errors
                .get(&task_id)
                .and_then(|errors| errors.last())
                .cloned(),
        }
    }
}

/// 任务恢复统计信息
#[derive(Debug, Clone)]
pub struct TaskRecoveryStats {
    pub task_id: Uuid,
    pub retry_count: u32,
    pub error_count: usize,
    pub checkpoint_count: usize,
    pub last_error: Option<String>,
}

/// 工作流级别的错误恢复
pub struct WorkflowRecoveryManager {
    recovery_manager: ErrorRecoveryManager,
    workflow_dependencies: HashMap<Uuid, Vec<Uuid>>, // workflow_id -> task_ids
}

impl WorkflowRecoveryManager {
    /// 创建新的工作流恢复管理器
    pub fn new(config: RecoveryConfig) -> Self {
        Self {
            recovery_manager: ErrorRecoveryManager::new(config),
            workflow_dependencies: HashMap::new(),
        }
    }

    /// 注册工作流任务
    pub fn register_workflow_task(&mut self, workflow_id: Uuid, task_id: Uuid) {
        self.workflow_dependencies
            .entry(workflow_id)
            .or_insert_with(Vec::new)
            .push(task_id);
    }

    /// 处理工作流级别的错误
    pub fn handle_workflow_error(
        &mut self,
        _workflow_id: Uuid,
        failed_task_id: Uuid,
        error: String,
        error_type: &str,
    ) -> Result<WorkflowRecoveryAction, AppError> {
        // 首先处理任务级别的错误
        let task_strategy = self.recovery_manager.handle_task_failure(
            failed_task_id,
            error,
            error_type,
        )?;

        // 根据任务恢复策略决定工作流级别的动作
        match task_strategy {
            RecoveryStrategy::StopWorkflow => {
                // 停止整个工作流
                Ok(WorkflowRecoveryAction::StopWorkflow)
            }
            RecoveryStrategy::SkipAndContinue => {
                // 跳过失败的任务，继续执行其他任务
                Ok(WorkflowRecoveryAction::SkipTaskAndContinue)
            }
            RecoveryStrategy::RollbackToCheckpoint => {
                // 回滚到检查点
                Ok(WorkflowRecoveryAction::RollbackWorkflow)
            }
            _ => {
                // 其他策略不影响工作流级别
                Ok(WorkflowRecoveryAction::Continue)
            }
        }
    }

    /// 获取工作流中所有任务的恢复状态
    pub fn get_workflow_recovery_status(&self, workflow_id: Uuid) -> WorkflowRecoveryStatus {
        let task_ids = self.workflow_dependencies
            .get(&workflow_id)
            .cloned()
            .unwrap_or_default();

        let mut task_stats = Vec::new();
        let mut has_failures = false;
        let mut total_retries = 0;

        for task_id in task_ids {
            let stats = self.recovery_manager.get_task_stats(task_id);
            if stats.error_count > 0 {
                has_failures = true;
            }
            total_retries += stats.retry_count as u64;
            task_stats.push(stats);
        }

        WorkflowRecoveryStatus {
            workflow_id,
            task_count: task_stats.len(),
            has_failures,
            total_retries,
            task_stats,
        }
    }
}

/// 工作流恢复动作
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowRecoveryAction {
    /// 继续执行
    Continue,
    /// 跳过失败任务继续执行
    SkipTaskAndContinue,
    /// 回滚整个工作流
    RollbackWorkflow,
    /// 停止工作流
    StopWorkflow,
}

/// 工作流恢复状态
#[derive(Debug, Clone)]
pub struct WorkflowRecoveryStatus {
    pub workflow_id: Uuid,
    pub task_count: usize,
    pub has_failures: bool,
    pub total_retries: u64,
    pub task_stats: Vec<TaskRecoveryStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recovery_manager() {
        let config = RecoveryConfig {
            max_retries: 3,
            initial_retry_delay: 1,
            max_retry_delay: 10,
            strategy: RecoveryStrategy::ExponentialBackoff,
            enable_checkpoints: true,
            checkpoint_interval: 60,
        };

        let mut manager = ErrorRecoveryManager::new(config);
        let task_id = Uuid::new_v4();

        // 测试错误记录
        manager.record_error(task_id, "Test error 1".to_string());
        manager.record_error(task_id, "Test error 2".to_string());
        
        let errors = manager.get_task_errors(task_id);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0], "Test error 1");

        // 测试重试逻辑
        assert!(manager.should_retry(task_id)); // 第一次重试
        assert!(manager.should_retry(task_id)); // 第二次重试
        assert!(manager.should_retry(task_id)); // 第三次重试
        assert!(!manager.should_retry(task_id)); // 超过最大重试次数

        // 测试重试延迟计算
        manager.reset_task_retry_count(task_id);
        manager.should_retry(task_id);
        let delay = manager.get_retry_delay(task_id);
        assert_eq!(delay, 2); // 1 * 2^1 = 2
    }

    #[test]
    fn test_recovery_strategy_selection() {
        let manager = ErrorRecoveryManager::default();

        assert_eq!(
            manager.select_recovery_strategy("timeout"),
            RecoveryStrategy::ExponentialBackoff
        );
        assert_eq!(
            manager.select_recovery_strategy("data_corruption"),
            RecoveryStrategy::RollbackToCheckpoint
        );
        assert_eq!(
            manager.select_recovery_strategy("fatal_error"),
            RecoveryStrategy::StopWorkflow
        );
        assert_eq!(
            manager.select_recovery_strategy("unknown_error"),
            RecoveryStrategy::ExponentialBackoff // 默认策略
        );
    }

    #[test]
    fn test_workflow_recovery_manager() {
        let mut workflow_manager = WorkflowRecoveryManager::new(RecoveryConfig::default());
        let workflow_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();

        workflow_manager.register_workflow_task(workflow_id, task_id);

        let status = workflow_manager.get_workflow_recovery_status(workflow_id);
        assert_eq!(status.task_count, 1);
        assert!(!status.has_failures);
    }
}