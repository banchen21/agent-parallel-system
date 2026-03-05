use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::workspace::{
        Workspace, CreateWorkspaceRequest, UpdateWorkspaceRequest, WorkspaceResponse,
        WorkspacePermission, PermissionLevel, GrantPermissionRequest, Document, Tool,
    },
};

/// 工作空间服务
#[derive(Clone)]
pub struct WorkspaceService {
    db_pool: PgPool,
}

impl WorkspaceService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
    
    /// 创建工作空间
    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceRequest,
        user_id: Uuid,
    ) -> Result<Workspace, AppError> {
        let workspace = sqlx::query_as!(
            Workspace,
            r#"
            INSERT INTO workspaces (
                name, description, owner_id, is_public, context, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
            request.name,
            request.description,
            user_id,
            request.is_public.unwrap_or(false),
            request.context.unwrap_or(serde_json::Value::Null),
            request.metadata.unwrap_or(serde_json::Value::Null)
        )
        .fetch_one(&self.db_pool)
        .await
        .context("创建工作空间失败")?;
        
        Ok(workspace)
    }
    
    /// 根据ID获取工作空间
    pub async fn get_workspace(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Workspace>, AppError> {
        let workspace = sqlx::query_as!(
            Workspace,
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
        
        Ok(workspace)
    }
    
    /// 获取用户的工作空间列表
    pub async fn get_user_workspaces(
        &self,
        user_id: Uuid,
        page: Option<i64>,
        page_size: Option<i64>,
    ) -> Result<Vec<Workspace>, AppError> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20);
        let offset = (page - 1) * page_size;
        
        let workspaces = sqlx::query_as!(
            Workspace,
            r#"
            SELECT w.* FROM workspaces w
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id AND wp.user_id = $1
            WHERE w.owner_id = $1 OR w.is_public = true OR wp.user_id IS NOT NULL
            ORDER BY w.updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            page_size,
            offset
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(workspaces)
    }
    
    /// 更新工作空间
    pub async fn update_workspace(
        &self,
        workspace_id: Uuid,
        request: UpdateWorkspaceRequest,
        user_id: Uuid,
    ) -> Result<Workspace, AppError> {
        // 检查工作空间权限（必须是所有者）
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        let workspace = workspace.ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
        
        if workspace.owner_id != user_id {
            return Err(AppError::PermissionDenied("只有所有者可以更新工作空间".to_string()));
        }
        
        let updated_workspace = sqlx::query_as!(
            Workspace,
            r#"
            UPDATE workspaces 
            SET 
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                is_public = COALESCE($3, is_public),
                context = COALESCE($4, context),
                metadata = COALESCE($5, metadata),
                updated_at = NOW()
            WHERE id = $6
            RETURNING *
            "#,
            request.name,
            request.description,
            request.is_public,
            request.context,
            request.metadata,
            workspace_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("更新工作空间失败")?;
        
        Ok(updated_workspace)
    }
    
    /// 删除工作空间
    pub async fn delete_workspace(&self, workspace_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        // 检查工作空间权限（必须是所有者）
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        let workspace = workspace.ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
        
        if workspace.owner_id != user_id {
            return Err(AppError::PermissionDenied("只有所有者可以删除工作空间".to_string()));
        }
        
        sqlx::query!(
            "DELETE FROM workspaces WHERE id = $1",
            workspace_id
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(())
    }
    
    /// 授予工作空间权限
    pub async fn grant_permission(
        &self,
        workspace_id: Uuid,
        request: GrantPermissionRequest,
        granted_by: Uuid,
    ) -> Result<WorkspacePermission, AppError> {
        // 检查工作空间权限（必须是所有者）
        let workspace = self.get_workspace(workspace_id, granted_by).await?;
        let workspace = workspace.ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
        
        if workspace.owner_id != granted_by {
            return Err(AppError::PermissionDenied("只有所有者可以授予权限".to_string()));
        }
        
        // 确保只授予用户或智能体之一
        if request.user_id.is_some() && request.agent_id.is_some() {
            return Err(AppError::ValidationError(
                "只能授予用户或智能体权限，不能同时授予".to_string(),
            ));
        }
        
        if request.user_id.is_none() && request.agent_id.is_none() {
            return Err(AppError::ValidationError(
                "必须指定用户或智能体".to_string(),
            ));
        }
        
        let permission = sqlx::query_as!(
            WorkspacePermission,
            r#"
            INSERT INTO workspace_permissions (
                workspace_id, user_id, agent_id, permission_level, granted_by, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
            workspace_id,
            request.user_id,
            request.agent_id,
            request.permission_level.to_string(),
            granted_by,
            request.expires_at
        )
        .fetch_one(&self.db_pool)
        .await
        .context("授予权限失败")?;
        
        Ok(permission)
    }
    
    /// 撤销工作空间权限
    pub async fn revoke_permission(
        &self,
        permission_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        // 检查权限记录
        let permission = sqlx::query_as!(
            WorkspacePermission,
            r#"
            SELECT wp.*
            FROM workspace_permissions wp
            WHERE wp.id = $1
            "#,
            permission_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let permission = permission.ok_or_else(|| AppError::NotFound("权限记录不存在".to_string()))?;

        let workspace_owner = sqlx::query!(
            "SELECT owner_id FROM workspaces WHERE id = $1",
            permission.workspace_id
        )
        .fetch_one(&self.db_pool)
        .await?;
        
        // 检查权限（必须是所有者或权限授予者）
        if workspace_owner.owner_id != user_id && permission.granted_by != user_id {
            return Err(AppError::PermissionDenied("没有权限撤销此权限".to_string()));
        }
        
        sqlx::query!(
            "DELETE FROM workspace_permissions WHERE id = $1",
            permission_id
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(())
    }
    
    /// 获取工作空间权限列表
    pub async fn get_workspace_permissions(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<WorkspacePermission>, AppError> {
        // 检查工作空间权限
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        let workspace = workspace.ok_or_else(|| AppError::NotFound("工作空间不存在".to_string()))?;
        
        // 只有所有者可以查看权限列表
        if workspace.owner_id != user_id {
            return Err(AppError::PermissionDenied("只有所有者可以查看权限列表".to_string()));
        }
        
        let permissions = sqlx::query_as!(
            WorkspacePermission,
            "SELECT * FROM workspace_permissions WHERE workspace_id = $1",
            workspace_id
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(permissions)
    }
    
    /// 获取工作空间文档列表
    pub async fn get_workspace_documents(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Document>, AppError> {
        // 检查工作空间权限
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        if workspace.is_none() {
            return Err(AppError::PermissionDenied("没有访问该工作空间的权限".to_string()));
        }
        
        let documents = sqlx::query_as!(
            Document,
            "SELECT * FROM documents WHERE workspace_id = $1 ORDER BY created_at DESC",
            workspace_id
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(documents)
    }
    
    /// 获取工作空间工具列表
    pub async fn get_workspace_tools(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Tool>, AppError> {
        // 检查工作空间权限
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        if workspace.is_none() {
            return Err(AppError::PermissionDenied("没有访问该工作空间的权限".to_string()));
        }
        
        // 这里简化处理，实际应用中可能需要更复杂的工具权限检查
        let tools = sqlx::query_as!(
            Tool,
            "SELECT * FROM tools WHERE is_active = true ORDER BY name ASC",
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        Ok(tools)
    }
    
    /// 获取工作空间统计信息
    pub async fn get_workspace_stats(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        // 检查工作空间权限
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        if workspace.is_none() {
            return Err(AppError::PermissionDenied("没有访问该工作空间的权限".to_string()));
        }
        
        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(DISTINCT d.id) as document_count,
                COUNT(DISTINCT t.id) as task_count,
                COUNT(DISTINCT CASE WHEN t.status = 'in_progress' THEN t.id END) as active_task_count,
                COUNT(DISTINCT wp.id) as permission_count
            FROM workspaces w
            LEFT JOIN documents d ON w.id = d.workspace_id
            LEFT JOIN tasks t ON w.id = t.workspace_id
            LEFT JOIN workspace_permissions wp ON w.id = wp.workspace_id
            WHERE w.id = $1
            "#,
            workspace_id
        )
        .fetch_one(&self.db_pool)
        .await?;
        
        let stats_json = serde_json::json!({
            "document_count": stats.document_count.unwrap_or(0),
            "task_count": stats.task_count.unwrap_or(0),
            "active_task_count": stats.active_task_count.unwrap_or(0),
            "permission_count": stats.permission_count.unwrap_or(0),
        });
        
        Ok(stats_json)
    }
    
    /// 获取完整的工作空间响应
    pub async fn get_workspace_response(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<WorkspaceResponse>, AppError> {
        let workspace = self.get_workspace(workspace_id, user_id).await?;
        
        if let Some(workspace) = workspace {
            let permissions = self.get_workspace_permissions(workspace_id, user_id).await?;
            let permission_responses = permissions
                .into_iter()
                .map(|p| p.to_response())
                .collect();
            
            let stats = self.get_workspace_stats(workspace_id, user_id).await?;
            
            let response = WorkspaceResponse {
                id: workspace.id,
                name: workspace.name,
                description: workspace.description,
                owner_id: workspace.owner_id,
                is_public: workspace.is_public,
                context: workspace.context,
                metadata: workspace.metadata,
                created_at: workspace.created_at,
                updated_at: workspace.updated_at,
                permissions: permission_responses,
                document_count: stats["document_count"].as_i64().unwrap_or(0),
                active_task_count: stats["active_task_count"].as_i64().unwrap_or(0),
            };
            
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }
}
