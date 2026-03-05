use anyhow::{Result};
use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{
    core::{
        errors::AppError,
        dag::DagOrchestrator,
        error_recovery::{ErrorRecoveryManager, RecoveryStrategy},
    },
    models::{
        task::{Task, TaskStatus},
        agent::Agent,
    },
};

/// 编排器服务
#[derive(Clone)]
pub struct OrchestratorService {
    db_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
    dag_orchestrator: Arc<Mutex<DagOrchestrator>>,
    error_recovery_manager: Arc<Mutex<ErrorRecoveryManager>>,
}

impl OrchestratorService {
    pub fn new(db_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            db_pool,
            redis_pool,
            dag_orchestrator: Arc::new(Mutex::new(DagOrchestrator::new())),
            error_recovery_manager: Arc::new(Mutex::new(ErrorRecoveryManager::default())),
        }
    }
    
    /// 分配任务给合适的智能体
    pub async fn assign_task_to_best_agent(&self, task_id: Uuid) -> Result<bool, AppError> {
        // 获取任务信息
        let task = sqlx::query_as!(
            Task,
            "SELECT * FROM tasks WHERE id = $1",
            task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        
        // 检查任务状态
        if task.status != TaskStatus::Pending {
            return Err(AppError::ValidationError(
                "只能分配待处理的任务".to_string(),
            ));
        }
        
        // 从任务要求中提取所需能力
        let required_capabilities = self.extract_required_capabilities(&task).await?;
        
        // 查找合适的智能体
        let best_agent = self.find_best_agent_for_task(&required_capabilities, &task).await?;
        
        if let Some(agent) = best_agent {
            // 分配任务给智能体
            self.assign_task_to_specific_agent(&task, &agent).await?;
            
            // 发布任务分配事件
            self.publish_task_assignment_event(&task, &agent).await?;
            
            tracing::info!(
                "任务 {} 已分配给智能体 {}",
                task.id,
                agent.name
            );
            
            Ok(true)
        } else {
            tracing::warn!(
                "没有找到合适的智能体来处理任务 {}，所需能力: {:?}",
                task.id,
                required_capabilities
            );
            
            // 将任务标记为等待资源
            self.mark_task_as_waiting(&task).await?;
            
            Ok(false)
        }
    }
    
    /// 从任务要求中提取所需能力
    async fn extract_required_capabilities(&self, task: &Task) -> Result<Vec<String>, AppError> {
        // 从任务要求中提取能力
        let mut capabilities = Vec::new();
        
        if let Some(requirements) = task.requirements.as_object() {
            if let Some(caps) = requirements.get("capabilities") {
                if let Some(cap_list) = caps.as_array() {
                    for cap in cap_list {
                        if let Some(cap_str) = cap.as_str() {
                            capabilities.push(cap_str.to_string());
                        }
                    }
                }
            }
        }
        
        // 如果任务要求中没有明确指定能力，尝试从任务描述中推断
        if capabilities.is_empty() {
            capabilities = self.infer_capabilities_from_task(task).await?;
        }
        
        Ok(capabilities)
    }
    
    /// 从任务描述中推断能力
    async fn infer_capabilities_from_task(&self, task: &Task) -> Result<Vec<String>, AppError> {
        let mut capabilities = Vec::new();
        
        // 基于任务标题和描述的简单推断
        let task_text = format!("{} {}", task.title, task.description.as_deref().unwrap_or(""));
        let task_text = task_text.to_lowercase();
        
        // 关键词到能力的映射
        let keyword_mapping = vec![
            ("分析", "data_analysis"),
            ("报告", "report_writing"),
            ("数据", "data_processing"),
            ("代码", "code_generation"),
            ("翻译", "translation"),
            ("总结", "summarization"),
            ("写作", "content_writing"),
            ("研究", "research"),
            ("计算", "calculation"),
            ("分类", "classification"),
        ];
        
        for (keyword, capability) in keyword_mapping {
            if task_text.contains(keyword) {
                capabilities.push(capability.to_string());
            }
        }
        
        // 如果没有匹配到任何能力，使用默认能力
        if capabilities.is_empty() {
            capabilities.push("general_processing".to_string());
        }
        
        Ok(capabilities)
    }
    
    /// 查找最适合任务的智能体
    async fn find_best_agent_for_task(
        &self,
        required_capabilities: &[String],
        task: &Task,
    ) -> Result<Option<Agent>, AppError> {
        // 获取所有可用智能体
        let available_agents = sqlx::query_as!(
            Agent,
            r#"
            SELECT * FROM agents 
            WHERE status = 'online' 
            AND current_load < max_concurrent_tasks
            AND last_heartbeat > NOW() - INTERVAL '5 minutes'
            ORDER BY current_load ASC, last_heartbeat DESC
            "#
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        // 过滤具有所需能力的智能体
        let capable_agents: Vec<&Agent> = available_agents
            .iter()
            .filter(|agent| {
                required_capabilities
                    .iter()
                    .all(|cap| agent.has_capability(cap))
            })
            .collect();
        
        if capable_agents.is_empty() {
            return Ok(None);
        }
        
        // 选择最佳智能体（基于负载和优先级）
        let best_agent = self.select_best_agent(capable_agents, task).await?;
        
        Ok(Some(best_agent.clone()))
    }
    
    /// 选择最佳智能体
    async fn select_best_agent<'a>(
        &self,
        agents: Vec<&'a Agent>,
        _task: &Task,
    ) -> Result<&'a Agent, AppError> {
        // 简单的选择策略：选择负载最低的智能体
        // 在实际应用中，可以考虑更多因素：
        // - 任务优先级
        // - 智能体性能历史
        // - 任务类型匹配度
        // - 地理位置等
        
        agents
            .iter()
            .min_by_key(|agent| agent.current_load)
            .ok_or_else(|| AppError::InternalServerError)
            .map(|agent| *agent)
    }
    
    /// 将任务分配给特定智能体
    async fn assign_task_to_specific_agent(
        &self,
        task: &Task,
        agent: &Agent,
    ) -> Result<(), AppError> {
        // 更新任务状态和分配
        sqlx::query!(
            r#"
            UPDATE tasks 
            SET 
                assigned_agent_id = $1,
                status = 'in_progress',
                started_at = NOW(),
                updated_at = NOW()
            WHERE id = $2
            "#,
            agent.id,
            task.id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 更新智能体负载
        sqlx::query!(
            "UPDATE agents SET current_load = current_load + 1, updated_at = NOW() WHERE id = $1",
            agent.id
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(())
    }
    
    /// 发布任务分配事件
    async fn publish_task_assignment_event(
        &self,
        task: &Task,
        agent: &Agent,
    ) -> Result<(), AppError> {
        let mut conn = self.redis_pool.get().await?;
        
        let event_data = serde_json::json!({
            "event_type": "task_assigned",
            "task_id": task.id,
            "agent_id": agent.id,
            "timestamp": Utc::now().to_rfc3339(),
            "task_title": task.title,
            "agent_name": agent.name,
        });
        
        redis::cmd("PUBLISH")
            .arg("task_events")
            .arg(event_data.to_string())
            .query_async::<i64>(&mut *conn)
            .await?;
        
        Ok(())
    }
    
    /// 标记任务为等待资源
    async fn mark_task_as_waiting(&self, task: &Task) -> Result<(), AppError> {
        // 在实际应用中，这里可以设置重试机制或通知管理员
        // 目前只是记录日志
        
        tracing::warn!(
            "任务 {} 因缺少合适智能体而等待处理",
            task.id
        );
        
        Ok(())
    }
    
    /// 处理任务完成
    pub async fn handle_task_completion(
        &self,
        task_id: Uuid,
        result: Option<serde_json::Value>,
        success: bool,
    ) -> Result<(), AppError> {
        // 获取任务信息
        let task = sqlx::query_as!(
            Task,
            "SELECT * FROM tasks WHERE id = $1",
            task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        self.handle_task_completion_with_task(task, task_id, result, success).await
    }

    /// 由指定智能体回调任务完成（会校验任务归属）
    pub async fn handle_task_completion_by_agent(
        &self,
        agent_id: Uuid,
        task_id: Uuid,
        result: Option<serde_json::Value>,
        success: bool,
    ) -> Result<(), AppError> {
        let task = sqlx::query_as!(
            Task,
            "SELECT * FROM tasks WHERE id = $1",
            task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        if task.assigned_agent_id != Some(agent_id) {
            return Err(AppError::ValidationError(
                "任务不是由该智能体分配的".to_string(),
            ));
        }
        self.handle_task_completion_with_task(task, task_id, result, success).await
    }

    async fn handle_task_completion_with_task(
        &self,
        task: Task,
        task_id: Uuid,
        result: Option<serde_json::Value>,
        success: bool,
    ) -> Result<(), AppError> {
        
        // 更新任务状态
        let new_status = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        
        sqlx::query!(
            r#"
            UPDATE tasks 
            SET 
                status = $1,
                result = $2,
                completed_at = NOW(),
                execution_time = EXTRACT(EPOCH FROM (NOW() - started_at)),
                updated_at = NOW()
            WHERE id = $3
            "#,
            new_status.to_string(),
            result,
            task_id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 如果任务有分配的智能体，释放其负载
        if let Some(agent_id) = task.assigned_agent_id {
            sqlx::query!(
                "UPDATE agents SET current_load = GREATEST(0, current_load - 1), updated_at = NOW() WHERE id = $1",
                agent_id
            )
            .execute(&self.db_pool)
            .await?;
        }
        
        // 发布任务完成事件
        self.publish_task_completion_event(&task, success).await?;
        
        // 检查是否有依赖此任务的子任务可以开始
        if success {
            self.check_dependent_tasks(task_id).await?;
        }
        
        Ok(())
    }
    
    /// 发布任务完成事件
    async fn publish_task_completion_event(
        &self,
        task: &Task,
        success: bool,
    ) -> Result<(), AppError> {
        let mut conn = self.redis_pool.get().await?;
        
        let event_data = serde_json::json!({
            "event_type": if success { "task_completed" } else { "task_failed" },
            "task_id": task.id,
            "task_title": task.title,
            "timestamp": Utc::now().to_rfc3339(),
            "success": success,
        });
        
        redis::cmd("PUBLISH")
            .arg("task_events")
            .arg(event_data.to_string())
            .query_async::<i64>(&mut *conn)
            .await?;
        
        Ok(())
    }
    
    /// 检查依赖此任务的子任务
    pub async fn check_dependent_tasks(&self, task_id: Uuid) -> Result<(), AppError> {
        // 查找依赖此任务的其他任务
        let dependent_tasks = sqlx::query!(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies td ON t.id = td.task_id
            WHERE td.depends_on_task_id = $1
            AND t.status = 'pending'
            "#,
            task_id
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        for dependent_task in dependent_tasks {
            // 检查是否所有依赖都已完成
            let remaining_dependencies = sqlx::query!(
                r#"
                SELECT COUNT(*) as count
                FROM task_dependencies td
                INNER JOIN tasks t ON td.depends_on_task_id = t.id
                WHERE td.task_id = $1
                AND t.status != 'completed'
                "#,
                dependent_task.id
            )
            .fetch_one(&self.db_pool)
            .await?;
            
            if remaining_dependencies.count.unwrap_or(0) == 0 {
                // 所有依赖都已完成，可以开始此任务
                self.assign_task_to_best_agent(dependent_task.id).await?;
            }
        }
        
        Ok(())
    }
    
    /// 批量分配待处理任务
    pub async fn batch_assign_pending_tasks(&self) -> Result<u64, AppError> {
        // 获取所有待处理的任务
        let pending_tasks = sqlx::query_as!(
            Task,
            "SELECT * FROM tasks WHERE status = 'pending' ORDER BY priority DESC, created_at ASC LIMIT 100"
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        let mut assigned_count = 0;
        
        for task in pending_tasks {
            if self.assign_task_to_best_agent(task.id).await? {
                assigned_count += 1;
            }
        }
        
        tracing::info!("批量分配了 {} 个待处理任务", assigned_count);
        
        Ok(assigned_count)
    }
    
    /// 获取编排器统计信息
    pub async fn get_orchestrator_stats(&self) -> Result<serde_json::Value, AppError> {
        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_tasks,
                COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending_tasks,
                COUNT(CASE WHEN status = 'in_progress' THEN 1 END) as in_progress_tasks,
                COUNT(CASE WHEN status = 'completed' THEN 1 END) as completed_tasks,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed_tasks,
                AVG(execution_time)::float8 as avg_execution_time
            FROM tasks
            "#
        )
        .fetch_one(&self.db_pool)
        .await?;
        
        let stats_json = serde_json::json!({
            "total_tasks": stats.total_tasks.unwrap_or(0),
            "pending_tasks": stats.pending_tasks.unwrap_or(0),
            "in_progress_tasks": stats.in_progress_tasks.unwrap_or(0),
            "completed_tasks": stats.completed_tasks.unwrap_or(0),
            "failed_tasks": stats.failed_tasks.unwrap_or(0),
            "average_execution_time": stats.avg_execution_time.unwrap_or(0.0),
        });
        
        Ok(stats_json)
    }

    // ==================== DAG编排功能 ====================

    /// 使用DAG编排器处理复杂任务依赖
    pub async fn process_dag_workflow(&self, workflow_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        // 获取工作流中的所有任务
        let workflow_tasks = sqlx::query_as!(
            Task,
            "SELECT * FROM tasks WHERE workspace_id = $1 ORDER BY created_at ASC",
            workflow_id
        )
        .fetch_all(&self.db_pool)
        .await?;

        // 获取任务依赖关系
        let dependencies = sqlx::query!(
            r#"
            SELECT task_id, depends_on_task_id, dependency_type
            FROM task_dependencies
            WHERE task_id IN (
                SELECT id FROM tasks WHERE workspace_id = $1
            )"#,
            workflow_id
        )
        .fetch_all(&self.db_pool)
        .await?;

        let mut dag = self.dag_orchestrator.lock().unwrap();
        dag.clear();

        // 添加任务到DAG
        for task in workflow_tasks {
            dag.add_task(task);
        }

        // 添加依赖关系到DAG
        for dep in dependencies {
            if let Err(e) = dag.add_dependency(dep.task_id, dep.depends_on_task_id) {
                tracing::warn!("添加依赖失败: {} -> {}: {}", dep.task_id, dep.depends_on_task_id, e);
            }
        }

        // 获取执行顺序
        let execution_order = dag.get_execution_order()?;
        
        // 获取就绪的任务
        let ready_tasks = dag.get_ready_tasks();
        
        tracing::info!(
            "工作流 {} 的DAG编排完成，执行顺序: {:?}，就绪任务: {:?}",
            workflow_id,
            execution_order,
            ready_tasks
        );

        Ok(ready_tasks)
    }

    /// 检查并分配就绪的DAG任务
    pub async fn assign_ready_dag_tasks(&self, workflow_id: Uuid) -> Result<u64, AppError> {
        let ready_tasks = self.process_dag_workflow(workflow_id).await?;
        let mut assigned_count = 0;

        for task_id in ready_tasks {
            if self.assign_task_to_best_agent(task_id).await? {
                assigned_count += 1;
            }
        }

        tracing::info!("为工作流 {} 分配了 {} 个就绪的DAG任务", workflow_id, assigned_count);
        Ok(assigned_count)
    }

    /// 获取DAG编排状态
    pub async fn get_dag_status(&self, workflow_id: Uuid) -> Result<serde_json::Value, AppError> {
        let ready_tasks = self.process_dag_workflow(workflow_id).await?;
        let dag = self.dag_orchestrator.lock().unwrap();

        let execution_order = dag.get_execution_order()?;
        let critical_path = dag.get_critical_path()?;

        let status = serde_json::json!({
            "workflow_id": workflow_id,
            "total_tasks": dag.get_all_tasks().len(),
            "ready_tasks": ready_tasks,
            "execution_order": execution_order,
            "critical_path": critical_path,
            "has_cycles": execution_order.len() != dag.get_all_tasks().len(),
        });

        Ok(status)
    }

    // ==================== 错误恢复功能 ====================

    /// 处理任务失败并应用恢复策略
    pub async fn handle_task_failure_with_recovery(
        &self,
        task_id: Uuid,
        error: String,
        error_type: &str,
    ) -> Result<RecoveryStrategy, AppError> {
        let mut recovery_manager = self.error_recovery_manager.lock().unwrap();

        // 处理任务失败
        let strategy = recovery_manager.handle_task_failure(task_id, error, error_type)?;

        // 根据恢复策略执行相应操作
        match strategy {
            RecoveryStrategy::ImmediateRetry | RecoveryStrategy::ExponentialBackoff => {
                let delay = recovery_manager.get_retry_delay(task_id);
                tracing::info!("任务 {} 将在 {} 秒后重试，策略: {:?}", task_id, delay, strategy);
                
                // 在实际应用中，这里可以安排延迟重试
                // 目前只是记录日志
            }
            RecoveryStrategy::RollbackToCheckpoint => {
                if let Ok(checkpoint) = recovery_manager.rollback_to_checkpoint(task_id) {
                    tracing::info!("任务 {} 将回滚到检查点: {:?}", task_id, checkpoint);
                    
                    // 在实际应用中，这里可以执行回滚操作
                    // 目前只是记录日志
                }
            }
            RecoveryStrategy::SkipAndContinue => {
                tracing::warn!("任务 {} 将被跳过，继续执行其他任务", task_id);
                
                // 标记任务为跳过状态
                sqlx::query!(
                    "UPDATE tasks SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
                    task_id
                )
                .execute(&self.db_pool)
                .await?;
            }
            RecoveryStrategy::StopWorkflow => {
                tracing::error!("任务 {} 发生致命错误，停止整个工作流", task_id);
                
                // 在实际应用中，这里可以停止相关工作流
                // 目前只是记录日志
            }
        }

        Ok(strategy)
    }

    /// 创建任务检查点
    pub async fn create_task_checkpoint(&self, task_id: Uuid) -> Result<(), AppError> {
        let task = sqlx::query_as!(
            Task,
            "SELECT * FROM tasks WHERE id = $1",
            task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some(task) = task {
            let mut recovery_manager = self.error_recovery_manager.lock().unwrap();
            recovery_manager.create_checkpoint(&task);
            tracing::debug!("为任务 {} 创建了检查点", task_id);
        }

        Ok(())
    }

    /// 获取任务恢复统计信息
    pub async fn get_task_recovery_stats(&self, task_id: Uuid) -> Result<serde_json::Value, AppError> {
        let recovery_manager = self.error_recovery_manager.lock().unwrap();
        let stats = recovery_manager.get_task_stats(task_id);

        let stats_json = serde_json::json!({
            "task_id": stats.task_id,
            "retry_count": stats.retry_count,
            "error_count": stats.error_count,
            "checkpoint_count": stats.checkpoint_count,
            "last_error": stats.last_error,
        });

        Ok(stats_json)
    }

    /// 重置任务恢复状态
    pub async fn reset_task_recovery_state(&self, task_id: Uuid) -> Result<(), AppError> {
        let mut recovery_manager = self.error_recovery_manager.lock().unwrap();
        recovery_manager.cleanup_task_history(task_id);
        tracing::info!("已重置任务 {} 的恢复状态", task_id);
        Ok(())
    }
}
