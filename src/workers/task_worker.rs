use anyhow::Result;
use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use sqlx::PgPool;
use tokio::time::{self, Duration};
use tracing::{error, info};

use crate::{
    core::errors::AppError,
    services::{orchestrator_service::OrchestratorService, message_service::MessageService},
};

/// 任务工作器
pub struct TaskWorker {
    db_pool: PgPool,
    _redis_pool: bb8::Pool<RedisConnectionManager>,
    orchestrator_service: OrchestratorService,
    message_service: MessageService,
}

impl TaskWorker {
    pub fn new(
        db_pool: PgPool,
        redis_pool: bb8::Pool<RedisConnectionManager>,
    ) -> Self {
        let orchestrator_service = OrchestratorService::new(db_pool.clone(), redis_pool.clone());
        let message_service = MessageService::new(db_pool.clone(), redis_pool.clone());
        
        Self {
            db_pool,
            _redis_pool: redis_pool,
            orchestrator_service,
            message_service,
        }
    }
    
    /// 启动任务工作器
    pub async fn start(&self) -> Result<()> {
        info!("启动任务工作器...");
        
        // 启动任务分配循环
        let task_assignment_worker = self.start_task_assignment_worker();
        
        // 启动任务监控循环
        let task_monitoring_worker = self.start_task_monitoring_worker();
        
        // 启动任务清理循环
        let task_cleanup_worker = self.start_task_cleanup_worker();
        
        // 等待所有工作器完成
        tokio::try_join!(
            task_assignment_worker,
            task_monitoring_worker,
            task_cleanup_worker,
        )?;
        
        Ok(())
    }
    
    /// 启动任务分配工作器
    async fn start_task_assignment_worker(&self) -> Result<()> {
        info!("启动任务分配工作器");
        
        let mut interval = time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            match self.process_pending_tasks().await {
                Ok(assigned_count) => {
                    if assigned_count > 0 {
                        info!("分配了 {} 个待处理任务", assigned_count);
                    }
                }
                Err(e) => {
                    error!("处理待处理任务时出错: {}", e);
                }
            }
        }
    }
    
    /// 处理待处理任务
    async fn process_pending_tasks(&self) -> Result<u64, AppError> {
        self.orchestrator_service.batch_assign_pending_tasks().await
    }
    
    /// 启动任务监控工作器
    async fn start_task_monitoring_worker(&self) -> Result<()> {
        info!("启动任务监控工作器");
        
        let mut interval = time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            match self.monitor_running_tasks().await {
                Ok(()) => {
                    // 监控成功
                }
                Err(e) => {
                    error!("监控运行中任务时出错: {}", e);
                }
            }
        }
    }
    
    /// 监控运行中的任务
    async fn monitor_running_tasks(&self) -> Result<(), AppError> {
        // 检查长时间运行的任务
        let long_running_tasks = sqlx::query!(
            r#"
            SELECT t.*, a.name as agent_name
            FROM tasks t
            LEFT JOIN agents a ON t.assigned_agent_id = a.id
            WHERE t.status = 'in_progress'
            AND t.started_at < NOW() - INTERVAL '1 hour'
            AND t.updated_at < NOW() - INTERVAL '30 minutes'
            "#
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        for task in long_running_tasks {
            let running_minutes = task
                .started_at
                .map(|started| Utc::now().signed_duration_since(started).num_minutes())
                .unwrap_or(0);

            info!(
                "检测到长时间运行的任务: {} (ID: {}), 分配给: {}, 已运行: {}分钟",
                task.title,
                task.id,
                task.agent_name.unwrap_or("未知".to_string()),
                running_minutes
            );
            
            // 发送警告通知
            let _ = self.message_service.send_user_message(
                task.created_by,
                "SYSTEM",
                "任务运行时间过长",
                Some(serde_json::json!({
                    "task_id": task.id,
                    "workspace_id": task.workspace_id,
                    "task_title": task.title,
                    "warning": "任务运行时间过长",
                    "running_time_minutes": running_minutes,
                })),
            )
            .await;
        }
        
        // 检查超时任务
        let timeout_tasks = sqlx::query!(
            r#"
            SELECT t.* FROM tasks t
            WHERE t.status = 'in_progress'
            AND t.started_at < NOW() - INTERVAL '24 hours'
            "#
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        for task in timeout_tasks {
            info!("检测到超时任务: {} (ID: {})", task.title, task.id);
            
            // 标记任务为失败
            sqlx::query!(
                r#"
                UPDATE tasks 
                SET 
                    status = 'failed',
                    result = jsonb_build_object('error', '任务执行超时'),
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                "#,
                task.id
            )
            .execute(&self.db_pool)
            .await?;
            
            // 释放智能体负载
            if let Some(agent_id) = task.assigned_agent_id {
                sqlx::query!(
                    "UPDATE agents SET current_load = GREATEST(0, current_load - 1), updated_at = NOW() WHERE id = $1",
                    agent_id
                )
                .execute(&self.db_pool)
                .await?;
            }
        }
        
        Ok(())
    }
    
    /// 启动任务清理工作器
    async fn start_task_cleanup_worker(&self) -> Result<()> {
        info!("启动任务清理工作器");
        
        let mut interval = time::interval(Duration::from_secs(3600)); // 每小时一次
        
        loop {
            interval.tick().await;
            
            match self.cleanup_old_tasks().await {
                Ok(cleaned_count) => {
                    if cleaned_count > 0 {
                        info!("清理了 {} 个旧任务", cleaned_count);
                    }
                }
                Err(e) => {
                    error!("清理旧任务时出错: {}", e);
                }
            }
        }
    }
    
    /// 清理旧任务
    async fn cleanup_old_tasks(&self) -> Result<u64, AppError> {
        // 清理30天前已完成的任务
        let result = sqlx::query!(
            r#"
            DELETE FROM tasks 
            WHERE status IN ('completed', 'failed', 'cancelled')
            AND completed_at < NOW() - INTERVAL '30 days'
            "#
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(result.rows_affected())
    }
    
    /// 处理任务事件
    pub async fn handle_task_event(&self, event_data: serde_json::Value) -> Result<(), AppError> {
        let event_type = event_data["event_type"]
            .as_str()
            .ok_or_else(|| AppError::ValidationError("事件类型缺失".to_string()))?;
        
        match event_type {
            "task_assigned" => {
                let task_id = event_data["task_id"]
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .ok_or_else(|| AppError::ValidationError("任务ID无效".to_string()))?;
                
                info!("处理任务分配事件: {}", task_id);
                
                // 这里可以添加额外的处理逻辑
                // 例如：发送通知、更新统计等
            }
            "task_completed" => {
                let task_id = event_data["task_id"]
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .ok_or_else(|| AppError::ValidationError("任务ID无效".to_string()))?;
                
                info!("处理任务完成事件: {}", task_id);
                
                // 检查依赖任务
                self.orchestrator_service.check_dependent_tasks(task_id).await?;
            }
            "task_failed" => {
                let task_id = event_data["task_id"]
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .ok_or_else(|| AppError::ValidationError("任务ID无效".to_string()))?;
                
                info!("处理任务失败事件: {}", task_id);
                
                // 这里可以添加失败处理逻辑
                // 例如：重试、通知管理员等
            }
            _ => {
                info!("忽略未知事件类型: {}", event_type);
            }
        }
        
        Ok(())
    }
}

pub async fn start_worker(
    db_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
) -> Result<()> {
    let worker = TaskWorker::new(db_pool, redis_pool);
    worker.start().await
}
