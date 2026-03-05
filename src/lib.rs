pub mod core;
pub mod models;
pub mod services;
pub mod api;
pub mod middleware;
pub mod utils;
pub mod workers;

use std::sync::Arc;

use sqlx::PgPool;
use bb8_redis::RedisConnectionManager;

use crate::{
    services::{
        auth_service::AuthService,
        task_service::TaskService,
        agent_service::AgentService,
        workspace_service::WorkspaceService,
        orchestrator_service::OrchestratorService,
    },
};

/// 应用状态，包含所有服务实例
#[derive(Clone)]
pub struct AppState {
    pub auth_service: Arc<AuthService>,
    pub task_service: Arc<TaskService>,
    pub agent_service: Arc<AgentService>,
    pub workspace_service: Arc<WorkspaceService>,
    pub orchestrator_service: Arc<OrchestratorService>,
    pub db_pool: PgPool,
    pub redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl AppState {
    pub fn new(db_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        let auth_service = Arc::new(AuthService::new(db_pool.clone()));
        let task_service = Arc::new(TaskService::new(db_pool.clone()));
        let agent_service = Arc::new(AgentService::new(db_pool.clone()));
        let workspace_service = Arc::new(WorkspaceService::new(db_pool.clone()));
        let orchestrator_service = Arc::new(OrchestratorService::new(
            db_pool.clone(),
            redis_pool.clone(),
        ));

        Self {
            auth_service,
            task_service,
            agent_service,
            workspace_service,
            orchestrator_service,
            db_pool,
            redis_pool,
        }
    }
}