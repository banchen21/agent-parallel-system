pub mod routes;

// 重新导出路由
pub use routes::{
    ui_routes,
    health_routes, auth_routes, task_routes, agent_routes, 
    workspace_routes, workflow_routes,
};
