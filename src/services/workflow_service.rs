use anyhow::Result;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::workflow::{
        CreateWorkflowRequest, ExecuteWorkflowRequest, Workflow, WorkflowExecution,
    },
};

#[derive(Clone)]
pub struct WorkflowService {
    db_pool: PgPool,
}

impl WorkflowService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    async fn ensure_workspace_access(&self, workspace_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let exists = sqlx::query(
            r#"
            SELECT 1
            FROM workspaces w
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $2
            WHERE w.id = $1
              AND (w.owner_id = $2 OR w.is_public = true OR wp.user_id IS NOT NULL)
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&self.db_pool)
        .await?;

        if exists.is_none() {
            return Err(AppError::PermissionDenied("没有访问该工作空间的权限".to_string()));
        }

        Ok(())
    }

    pub async fn create_workflow(
        &self,
        request: CreateWorkflowRequest,
        user_id: Uuid,
    ) -> Result<Workflow, AppError> {
        request.validate()?;
        self.ensure_workspace_access(request.workspace_id, user_id).await?;

        let workflow = sqlx::query_as::<_, Workflow>(
            r#"
            INSERT INTO workflows (name, description, workspace_id, definition, is_active, created_by)
            VALUES ($1, $2, $3, $4, true, $5)
            RETURNING id, name, description, workspace_id, definition, is_active, created_by, created_at, updated_at
            "#,
        )
        .bind(request.name)
        .bind(request.description)
        .bind(request.workspace_id)
        .bind(request.definition)
        .bind(user_id)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(workflow)
    }

    pub async fn list_workflows(
        &self,
        user_id: Uuid,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<Workflow>, AppError> {
        let workflows = sqlx::query_as::<_, Workflow>(
            r#"
            SELECT wf.id, wf.name, wf.description, wf.workspace_id, wf.definition, wf.is_active, wf.created_by, wf.created_at, wf.updated_at
            FROM workflows wf
            INNER JOIN workspaces w ON wf.workspace_id = w.id
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $1
            WHERE (w.owner_id = $1 OR w.is_public = true OR wp.user_id IS NOT NULL)
              AND ($2::uuid IS NULL OR wf.workspace_id = $2)
            ORDER BY wf.updated_at DESC
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(workflows)
    }

    pub async fn get_workflow(
        &self,
        workflow_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Workflow>, AppError> {
        let workflow = sqlx::query_as::<_, Workflow>(
            r#"
            SELECT wf.id, wf.name, wf.description, wf.workspace_id, wf.definition, wf.is_active, wf.created_by, wf.created_at, wf.updated_at
            FROM workflows wf
            INNER JOIN workspaces w ON wf.workspace_id = w.id
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $2
            WHERE wf.id = $1
              AND (w.owner_id = $2 OR w.is_public = true OR wp.user_id IS NOT NULL)
            "#,
        )
        .bind(workflow_id)
        .bind(user_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(workflow)
    }

    pub async fn delete_workflow(&self, workflow_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let owner = sqlx::query(
            r#"
            SELECT wf.created_by, w.owner_id
            FROM workflows wf
            INNER JOIN workspaces w ON wf.workspace_id = w.id
            WHERE wf.id = $1
            "#,
        )
        .bind(workflow_id)
        .fetch_optional(&self.db_pool)
        .await?;

        let owner = owner.ok_or_else(|| AppError::NotFound("工作流不存在".to_string()))?;
        let created_by: Uuid = owner.get("created_by");
        let owner_id: Uuid = owner.get("owner_id");

        if created_by != user_id && owner_id != user_id {
            return Err(AppError::PermissionDenied("没有权限删除该工作流".to_string()));
        }

        sqlx::query("DELETE FROM workflows WHERE id = $1")
            .bind(workflow_id)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    pub async fn create_execution(
        &self,
        workflow_id: Uuid,
        user_id: Uuid,
        request: ExecuteWorkflowRequest,
    ) -> Result<WorkflowExecution, AppError> {
        let workflow = self
            .get_workflow(workflow_id, user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("工作流不存在".to_string()))?;

        if !workflow.is_active {
            return Err(AppError::ValidationError("工作流已禁用，无法执行".to_string()));
        }

        let execution = sqlx::query_as::<_, WorkflowExecution>(
            r#"
            INSERT INTO workflow_executions (
                workflow_id, triggered_by, input, options, status, started_at
            )
            VALUES ($1, $2, $3, $4, 'queued', NOW())
            RETURNING id, workflow_id, triggered_by, input, options, status, task_id, result, error_message, started_at, completed_at, created_at, updated_at
            "#,
        )
        .bind(workflow_id)
        .bind(user_id)
        .bind(request.input.unwrap_or(serde_json::json!({})))
        .bind(request.options.unwrap_or(serde_json::json!({})))
        .fetch_one(&self.db_pool)
        .await?;

        Ok(execution)
    }

    pub async fn mark_execution_dispatched(
        &self,
        execution_id: Uuid,
        task_id: Uuid,
        assigned: bool,
    ) -> Result<WorkflowExecution, AppError> {
        let status = if assigned { "running" } else { "queued" };

        let execution = sqlx::query_as::<_, WorkflowExecution>(
            r#"
            UPDATE workflow_executions
            SET task_id = $2, status = $3, updated_at = NOW()
            WHERE id = $1
            RETURNING id, workflow_id, triggered_by, input, options, status, task_id, result, error_message, started_at, completed_at, created_at, updated_at
            "#,
        )
        .bind(execution_id)
        .bind(task_id)
        .bind(status)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(execution)
    }

    pub async fn mark_execution_failed(
        &self,
        execution_id: Uuid,
        err_msg: String,
    ) -> Result<WorkflowExecution, AppError> {
        let execution = sqlx::query_as::<_, WorkflowExecution>(
            r#"
            UPDATE workflow_executions
            SET status = 'failed', error_message = $2, completed_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING id, workflow_id, triggered_by, input, options, status, task_id, result, error_message, started_at, completed_at, created_at, updated_at
            "#,
        )
        .bind(execution_id)
        .bind(err_msg)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(execution)
    }

    pub async fn get_execution(
        &self,
        workflow_id: Uuid,
        execution_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<WorkflowExecution>, AppError> {
        let execution = sqlx::query_as::<_, WorkflowExecution>(
            r#"
            SELECT we.id, we.workflow_id, we.triggered_by, we.input, we.options, we.status, we.task_id, we.result, we.error_message, we.started_at, we.completed_at, we.created_at, we.updated_at
            FROM workflow_executions we
            INNER JOIN workflows wf ON we.workflow_id = wf.id
            INNER JOIN workspaces w ON wf.workspace_id = w.id
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $3
            WHERE we.id = $1
              AND we.workflow_id = $2
              AND (w.owner_id = $3 OR w.is_public = true OR wp.user_id IS NOT NULL)
            "#,
        )
        .bind(execution_id)
        .bind(workflow_id)
        .bind(user_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(execution)
    }
}
