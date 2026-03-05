use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::agent::{
        Agent, RegisterAgentRequest, AgentHeartbeatRequest, AgentResponse, AgentStatus,
        AgentHealthStatus, TaskAssignmentRequest, TaskAssignmentResponse,
    },
};

/// 智能体服务
#[derive(Clone)]
pub struct AgentService {
    db_pool: PgPool,
}

impl AgentService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
    
    /// 注册智能体
    pub async fn register_agent(&self, request: RegisterAgentRequest) -> Result<Agent, AppError> {
        // 验证请求
        request.validate()?;
        
        // 检查智能体名称是否已存在
        let existing_agent = sqlx::query!(
            "SELECT id FROM agents WHERE name = $1",
            request.name
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if existing_agent.is_some() {
            return Err(AppError::ValidationError(
                "智能体名称已存在".to_string(),
            ));
        }
        
        // 序列化配置
        let capabilities = serde_json::to_value(request.capabilities)
            .context("序列化智能体能力失败")?;
        let endpoints = serde_json::to_value(request.endpoints)
            .context("序列化智能体端点失败")?;
        let limits = serde_json::to_value(request.limits)
            .context("序列化智能体限制失败")?;
        let max_concurrent_tasks = limits
            .get("max_concurrent_tasks")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;
        
        let agent = sqlx::query_as!(
            Agent,
            r#"
            INSERT INTO agents (
                name, description, status, capabilities, endpoints, 
                limits, max_concurrent_tasks, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
            request.name,
            request.description,
            AgentStatus::Online as AgentStatus,
            capabilities,
            endpoints,
            limits,
            max_concurrent_tasks,
            request.metadata.unwrap_or(serde_json::Value::Null)
        )
        .fetch_one(&self.db_pool)
        .await
        .context("注册智能体失败")?;
        
        // 记录智能体注册日志
        crate::core::logging::log_agent_activity(
            &agent.id.to_string(),
            "register",
            None,
            Some(&format!("智能体 {} 注册成功", agent.name)),
        );
        
        Ok(agent)
    }
    
    /// 更新智能体心跳
    pub async fn update_heartbeat(
        &self,
        agent_id: Uuid,
        request: AgentHeartbeatRequest,
    ) -> Result<AgentHealthStatus, AppError> {
        // 验证请求
        request.validate()?;
        
        // 更新智能体状态
        let agent = sqlx::query_as!(
            Agent,
            r#"
            UPDATE agents 
            SET 
                current_load = $1,
                last_heartbeat = NOW(),
                metadata = COALESCE($2, metadata),
                updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
            request.current_load,
            request.metadata,
            agent_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let agent = agent.ok_or_else(|| AppError::NotFound("智能体不存在".to_string()))?;
        
        // 构建健康状态响应
        let health_status = AgentHealthStatus {
            status: "HEALTHY".to_string(),
            assigned_tasks: request.active_tasks,
            system_info: crate::models::agent::SystemInfo {
                maintenance_window: None,
                rate_limits: crate::models::agent::RateLimits {
                    max_requests_per_minute: 60,
                    max_tokens_per_minute: None,
                    max_concurrent_requests: None,
                },
                supported_models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
            },
        };
        
        Ok(health_status)
    }
    
    /// 获取所有可用智能体
    pub async fn get_available_agents(
        &self,
        capabilities: Option<Vec<String>>,
    ) -> Result<Vec<Agent>, AppError> {
        let mut query = "
            SELECT * FROM agents 
            WHERE status = 'online' 
            AND current_load < max_concurrent_tasks
            AND last_heartbeat > NOW() - INTERVAL '5 minutes'
        ".to_string();
        
        if let Some(caps) = capabilities {
            // 这里简化处理，实际应用中需要更复杂的JSON查询
            query.push_str(&format!(" AND capabilities::text LIKE '%{}%'", caps.join("%")));
        }
        
        query.push_str(" ORDER BY current_load ASC, last_heartbeat DESC");
        
        let agents = sqlx::query_as::<_, Agent>(&query)
            .fetch_all(&self.db_pool)
            .await?;
        
        Ok(agents)
    }
    
    /// 根据能力获取最佳智能体
    pub async fn get_best_agent_for_capabilities(
        &self,
        required_capabilities: &[String],
    ) -> Result<Option<Agent>, AppError> {
        let available_agents = self.get_available_agents(Some(required_capabilities.to_vec())).await?;
        
        // 选择负载最低的智能体
        let best_agent = available_agents
            .into_iter()
            .min_by_key(|agent| agent.current_load);
        
        Ok(best_agent)
    }
    
    /// 根据ID获取智能体
    pub async fn get_agent_by_id(&self, agent_id: Uuid) -> Result<Option<Agent>, AppError> {
        let agent = sqlx::query_as!(
            Agent,
            "SELECT * FROM agents WHERE id = $1",
            agent_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        Ok(agent)
    }
    
    /// 分配任务给智能体
    pub async fn assign_task_to_agent(
        &self,
        request: TaskAssignmentRequest,
    ) -> Result<TaskAssignmentResponse, AppError> {
        // 检查智能体是否存在且可用
        let agent = self.get_agent_by_id(request.agent_id).await?;
        let agent = agent.ok_or_else(|| AppError::NotFound("智能体不存在".to_string()))?;
        
        if !agent.is_available() {
            return Err(AppError::AgentError("智能体不可用".to_string()));
        }
        
        // 检查任务是否存在
        let task_exists = sqlx::query!(
            "SELECT id FROM tasks WHERE id = $1",
            request.task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if task_exists.is_none() {
            return Err(AppError::NotFound("任务不存在".to_string()));
        }
        
        // 更新任务分配
        sqlx::query!(
            "UPDATE tasks SET assigned_agent_id = $1, updated_at = NOW() WHERE id = $2",
            request.agent_id,
            request.task_id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 更新智能体负载
        sqlx::query!(
            "UPDATE agents SET current_load = current_load + 1, updated_at = NOW() WHERE id = $1",
            request.agent_id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 记录任务分配日志
        crate::core::logging::log_agent_activity(
            &request.agent_id.to_string(),
            "task_assigned",
            Some(&request.task_id.to_string()),
            Some(&format!("任务 {} 分配给智能体", request.task_id)),
        );
        
        Ok(TaskAssignmentResponse {
            success: true,
            assignment_id: Uuid::new_v4(),
            estimated_completion: Some(Utc::now() + chrono::Duration::minutes(30)),
            message: Some("任务分配成功".to_string()),
        })
    }
    
    /// 完成任务并释放智能体
    pub async fn complete_task_and_release_agent(
        &self,
        agent_id: Uuid,
        task_id: Uuid,
    ) -> Result<(), AppError> {
        // 检查智能体和任务关联
        let task = sqlx::query!(
            "SELECT assigned_agent_id FROM tasks WHERE id = $1",
            task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        if let Some(task) = task {
            if task.assigned_agent_id != Some(agent_id) {
                return Err(AppError::ValidationError(
                    "任务不是由该智能体分配的".to_string(),
                ));
            }
        } else {
            return Err(AppError::NotFound("任务不存在".to_string()));
        }
        
        // 释放智能体负载
        sqlx::query!(
            "UPDATE agents SET current_load = GREATEST(0, current_load - 1), updated_at = NOW() WHERE id = $1",
            agent_id
        )
        .execute(&self.db_pool)
        .await?;
        
        // 记录任务完成日志
        crate::core::logging::log_agent_activity(
            &agent_id.to_string(),
            "task_completed",
            Some(&task_id.to_string()),
            Some("任务完成，智能体已释放"),
        );
        
        Ok(())
    }
    
    /// 更新智能体状态
    pub async fn update_agent_status(
        &self,
        agent_id: Uuid,
        status: AgentStatus,
    ) -> Result<Agent, AppError> {
        let agent = sqlx::query_as!(
            Agent,
            r#"
            UPDATE agents 
            SET status = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING *
            "#,
            status.to_string(),
            agent_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let agent = agent.ok_or_else(|| AppError::NotFound("智能体不存在".to_string()))?;
        
        // 记录状态更新日志
        crate::core::logging::log_agent_activity(
            &agent_id.to_string(),
            "status_update",
            None,
            Some(&format!("状态更新为: {:?}", status)),
        );
        
        Ok(agent)
    }
    
    /// 获取智能体统计信息
    pub async fn get_agent_stats(&self) -> Result<serde_json::Value, AppError> {
        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_agents,
                COUNT(CASE WHEN status = 'online' THEN 1 END) as online_agents,
                COUNT(CASE WHEN status = 'busy' THEN 1 END) as busy_agents,
                COUNT(CASE WHEN status = 'idle' THEN 1 END) as idle_agents,
                COUNT(CASE WHEN status = 'offline' THEN 1 END) as offline_agents,
                AVG(current_load)::float8 as avg_load,
                SUM(current_load) as total_load
            FROM agents
            "#
        )
        .fetch_one(&self.db_pool)
        .await?;
        
        let stats_json = serde_json::json!({
            "total_agents": stats.total_agents.unwrap_or(0),
            "online_agents": stats.online_agents.unwrap_or(0),
            "busy_agents": stats.busy_agents.unwrap_or(0),
            "idle_agents": stats.idle_agents.unwrap_or(0),
            "offline_agents": stats.offline_agents.unwrap_or(0),
            "average_load": stats.avg_load.unwrap_or(0.0),
            "total_load": stats.total_load.unwrap_or(0),
        });
        
        Ok(stats_json)
    }
    
    /// 清理离线智能体
    pub async fn cleanup_offline_agents(&self) -> Result<u64, AppError> {
        // 标记长时间没有心跳的智能体为离线
        let result = sqlx::query!(
            r#"
            UPDATE agents 
            SET status = 'offline', updated_at = NOW()
            WHERE status = 'online' 
            AND last_heartbeat < NOW() - INTERVAL '10 minutes'
            "#
        )
        .execute(&self.db_pool)
        .await?;
        
        let affected = result.rows_affected();
        
        if affected > 0 {
            tracing::info!("清理了 {} 个离线智能体", affected);
        }
        
        Ok(affected)
    }
}
