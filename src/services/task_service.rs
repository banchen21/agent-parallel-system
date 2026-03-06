use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::task::{
        Task, CreateTaskRequest, UpdateTaskRequest, UpdateTaskStatusRequest, TaskResponse,
        TaskStatus, TaskPriority, TaskDecompositionRequest, TaskDecompositionResult,
        SubtaskDefinition,
    },
};

/// 任务服务
#[derive(Clone)]
pub struct TaskService {
    db_pool: PgPool,
}

impl TaskService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
    
    /// 创建任务
    pub async fn create_task(
        &self,
        request: CreateTaskRequest,
        user_id: Uuid,
    ) -> Result<Task, AppError> {
        // 验证请求
        request.validate()?;
        
        // 创建工作空间权限检查（这里简化处理）
        // 在实际应用中，需要检查用户是否有权限在该工作空间创建任务
        
        let task = sqlx::query_as!(
            Task,
            r#"
            INSERT INTO tasks (
                title, description, status, priority, workspace_id,
                created_by, requirements, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
            request.title,
            request.description,
            "pending",
            request.priority.to_string(),
            request.workspace_id,
            user_id,
            request.requirements,
            request.metadata.unwrap_or(serde_json::Value::Null)
        )
        .fetch_one(&self.db_pool)
        .await
        .context("创建任务失败")?;
        
        // 记录任务创建日志
        crate::core::logging::log_task_execution(
            &task.id.to_string(),
            "create",
            "pending",
            None,
            None,
        );
        
        Ok(task)
    }
    
    /// 根据ID获取任务
    pub async fn get_task_by_id(&self, task_id: Uuid, user_id: Uuid) -> Result<Option<Task>, AppError> {
        let task = sqlx::query_as!(
            Task,
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN workspaces w ON t.workspace_id = w.id
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $2
            WHERE t.id = $1 
            AND (w.owner_id = $2 OR w.is_public = true OR wp.user_id IS NOT NULL)
            "#,
            task_id,
            user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        Ok(task)
    }
    
    /// 获取工作空间中的任务列表
    pub async fn get_tasks_by_workspace(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        status_filter: Option<TaskStatus>,
        priority_filter: Option<TaskPriority>,
        page: Option<i64>,
        page_size: Option<i64>,
    ) -> Result<Vec<Task>, AppError> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20);
        let offset = (page - 1) * page_size;
        
        // 检查工作空间权限
        let workspace = sqlx::query!(
            r#"
            SELECT w.* FROM workspaces w
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $2
            WHERE w.id = $1 
            AND (w.owner_id = $2 OR w.is_public = true OR wp.user_id IS NOT NULL)
            "#,
            workspace_id,
            user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if workspace.is_none() {
            return Err(AppError::PermissionDenied("没有访问该工作空间的权限".to_string()));
        }
        
        let tasks = sqlx::query_as!(
            Task,
            r#"
            SELECT t.* FROM tasks t
            WHERE t.workspace_id = $1
              AND ($2::text IS NULL OR t.status::text = $2)
              AND ($3::text IS NULL OR t.priority::text = $3)
            ORDER BY t.created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            workspace_id,
            status_filter.map(|s| s.to_string()),
            priority_filter.map(|p| p.to_string()),
            page_size,
            offset
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(tasks)
    }
    
    /// 更新任务状态
    pub async fn update_task_status(
        &self,
        task_id: Uuid,
        request: UpdateTaskStatusRequest,
        user_id: Uuid,
    ) -> Result<Task, AppError> {
        // 检查任务权限
        let task = self.get_task_by_id(task_id, user_id).await?;
        let task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        
        let now = Utc::now();
        let mut started_at = task.started_at;
        let mut completed_at = task.completed_at;
        
        // 根据状态更新相关时间字段
        let status_str = request.status.to_string();
        if status_str == "in_progress" && task.status == "pending" {
            started_at = Some(now);
        } else if status_str == "completed" || status_str == "failed" || status_str == "cancelled" {
            if started_at.is_some() && completed_at.is_none() {
                completed_at = Some(now);
            }
        }
        
        let updated_task = sqlx::query_as!(
            Task,
            r#"
            UPDATE tasks
            SET
                status = $1,
                result = COALESCE($2, result),
                metadata = COALESCE($3, metadata),
                started_at = COALESCE($4, started_at),
                completed_at = COALESCE($5, completed_at),
                updated_at = NOW()
            WHERE id = $6
            RETURNING *
            "#,
            status_str,
            request.result,
            request.metadata,
            started_at,
            completed_at,
            task_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("更新任务状态失败")?;
        
        // 记录状态更新日志
        crate::core::logging::log_task_execution(
            &task_id.to_string(),
            "update_status",
            &request.status.to_string(),
            None,
            None,
        );
        
        Ok(updated_task)
    }
    
    /// 更新任务信息
    pub async fn update_task(
        &self,
        task_id: Uuid,
        request: UpdateTaskRequest,
        user_id: Uuid,
    ) -> Result<Task, AppError> {
        // 验证请求
        request.validate()?;
        
        // 检查任务权限
        let task = self.get_task_by_id(task_id, user_id).await?;
        let _task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        
        let updated_task = sqlx::query_as!(
            Task,
            r#"
            UPDATE tasks
            SET
                title = COALESCE($1, title),
                description = COALESCE($2, description),
                status = COALESCE($3, status),
                priority = COALESCE($4, priority),
                result = COALESCE($5, result),
                metadata = COALESCE($6, metadata),
                updated_at = NOW()
            WHERE id = $7
            RETURNING *
            "#,
            request.title,
            request.description,
            request.status.map(|s| s.to_string()),
            request.priority.map(|p| p.to_string()),
            request.result,
            request.metadata,
            task_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("更新任务失败")?;
        
        Ok(updated_task)
    }
    
    /// 删除任务
    pub async fn delete_task(&self, task_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        // 检查任务权限
        let task = self.get_task_by_id(task_id, user_id).await?;
        let task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        
        // 检查任务状态，只有待处理或失败的任务可以删除
        if task.status != "pending" && task.status != "failed" {
            return Err(AppError::ValidationError(
                "只能删除待处理或失败的任务".to_string(),
            ));
        }
        
        sqlx::query!(
            "DELETE FROM tasks WHERE id = $1",
            task_id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 记录删除日志
        crate::core::logging::log_task_execution(
            &task_id.to_string(),
            "delete",
            "deleted",
            None,
            None,
        );
        
        Ok(())
    }
    
    /// 任务分解
    pub async fn decompose_task(
        &self,
        task_id: Uuid,
        request: TaskDecompositionRequest,
        user_id: Uuid,
    ) -> Result<TaskDecompositionResult, AppError> {
        // 检查任务权限
        let task = self.get_task_by_id(task_id, user_id).await?;
        let task = task.ok_or_else(|| AppError::NotFound("任务不存在".to_string()))?;
        
        // 这里实现任务分解逻辑
        // 在实际应用中，这里会调用LLM或使用预定义的分解策略
        let subtasks = self.generate_subtasks(&task, &request).await?;
        
        // 创建子任务
        let mut created_subtasks = Vec::new();
        for subtask_def in &subtasks {
            let subtask_request = CreateTaskRequest {
                title: subtask_def.title.clone(),
                description: subtask_def.description.clone(),
                priority: subtask_def.priority.clone(),
                workspace_id: task.workspace_id,
                requirements: subtask_def.requirements.clone(),
                metadata: Some(serde_json::json!({
                    "parent_task_id": task_id
                })),
            };
            
            let subtask = self.create_task(subtask_request, user_id).await?;
            created_subtasks.push(subtask);
        }
        
        Ok(TaskDecompositionResult {
            parent_task_id: task_id,
            subtasks: created_subtasks,
        })
    }
    
    /// 生成子任务（简化实现）
    async fn generate_subtasks(
        &self,
        task: &Task,
        _request: &TaskDecompositionRequest,
    ) -> Result<Vec<SubtaskDefinition>, AppError> {
        // 这里是一个简化的实现
        // 在实际应用中，这里会调用LLM来分析任务并生成子任务
        
        // 使用请求中提供的子任务定义
        let subtasks = vec![
            SubtaskDefinition {
                title: format!("{} - 数据收集", task.title),
                description: Some("收集任务所需的数据和资源".to_string()),
                priority: TaskPriority::Medium,
                requirements: serde_json::json!({"capabilities": ["data_collection"]}),
            },
            SubtaskDefinition {
                title: format!("{} - 数据分析", task.title),
                description: Some("分析收集到的数据".to_string()),
                priority: TaskPriority::Medium,
                requirements: serde_json::json!({"capabilities": ["data_analysis"]}),
            },
            SubtaskDefinition {
                title: format!("{} - 报告生成", task.title),
                description: Some("基于分析结果生成报告".to_string()),
                priority: TaskPriority::Medium,
                requirements: serde_json::json!({"capabilities": ["report_writing"]}),
            },
        ];
        
        Ok(subtasks)
    }
    
    /// 获取任务的子任务
    pub async fn get_subtasks(&self, task_id: Uuid, user_id: Uuid) -> Result<Vec<Task>, AppError> {
        let subtasks = sqlx::query_as!(
            Task,
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN workspaces w ON t.workspace_id = w.id
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $2
            WHERE t.parent_task_id = $1 
            AND (w.owner_id = $2 OR w.is_public = true OR wp.user_id IS NOT NULL)
            ORDER BY t.created_at ASC
            "#,
            task_id,
            user_id
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(subtasks)
    }
    
    /// 获取任务响应（包含子任务）
    pub async fn get_task_response(&self, task_id: Uuid, user_id: Uuid) -> Result<Option<TaskResponse>, AppError> {
        let task = self.get_task_by_id(task_id, user_id).await?;
        
        if let Some(task) = task {
            let subtasks = self.get_subtasks(task_id, user_id).await?;
            let subtask_responses = subtasks
                .into_iter()
                .map(|t| t.to_response(vec![])) // 子任务不再递归获取子任务
                .collect();
            
            Ok(Some(task.to_response(subtask_responses)))
        } else {
            Ok(None)
        }
    }
}
