//! 通知工作器
//! 
//! 负责处理系统通知和消息推送

use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use crate::core::errors::AppError;

/// 通知消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub content: String,
    pub notification_type: String,
    pub priority: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: i64,
    pub read: bool,
}

/// 通知工作器
pub struct NotificationWorker {
    sender: mpsc::Sender<NotificationMessage>,
}

impl NotificationWorker {
    /// 创建新的通知工作器
    pub fn new() -> (Self, mpsc::Receiver<NotificationMessage>) {
        let (sender, receiver) = mpsc::channel(100);
        (Self { sender }, receiver)
    }

    /// 发送通知
    pub async fn send_notification(&self, message: NotificationMessage) -> Result<(), AppError> {
        self.sender
            .send(message)
            .await
            .map_err(|e| AppError::InternalError(e.to_string()))
    }

    /// 发送任务分配通知
    pub async fn send_task_assigned_notification(
        &self,
        user_id: String,
        task_id: String,
        task_name: String,
        agent_name: String,
    ) -> Result<(), AppError> {
        let message = NotificationMessage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            title: "任务已分配".to_string(),
            content: format!("任务 '{}' 已分配给智能体 '{}'", task_name, agent_name),
            notification_type: "task_assigned".to_string(),
            priority: "medium".to_string(),
            metadata: Some(serde_json::json!({
                "task_id": task_id,
                "task_name": task_name,
                "agent_name": agent_name
            })),
            created_at: chrono::Utc::now().timestamp_millis(),
            read: false,
        };

        self.send_notification(message).await
    }

    /// 发送任务完成通知
    pub async fn send_task_completed_notification(
        &self,
        user_id: String,
        task_id: String,
        task_name: String,
        result: Option<String>,
    ) -> Result<(), AppError> {
        let message = NotificationMessage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            title: "任务已完成".to_string(),
            content: format!("任务 '{}' 已完成", task_name),
            notification_type: "task_completed".to_string(),
            priority: "low".to_string(),
            metadata: Some(serde_json::json!({
                "task_id": task_id,
                "task_name": task_name,
                "result": result
            })),
            created_at: chrono::Utc::now().timestamp_millis(),
            read: false,
        };

        self.send_notification(message).await
    }

    /// 发送任务失败通知
    pub async fn send_task_failed_notification(
        &self,
        user_id: String,
        task_id: String,
        task_name: String,
        error_message: String,
    ) -> Result<(), AppError> {
        let message = NotificationMessage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            title: "任务失败".to_string(),
            content: format!("任务 '{}' 失败: {}", task_name, error_message),
            notification_type: "task_failed".to_string(),
            priority: "high".to_string(),
            metadata: Some(serde_json::json!({
                "task_id": task_id,
                "task_name": task_name,
                "error": error_message
            })),
            created_at: chrono::Utc::now().timestamp_millis(),
            read: false,
        };

        self.send_notification(message).await
    }

    /// 发送智能体状态变更通知
    pub async fn send_agent_status_changed_notification(
        &self,
        user_id: String,
        agent_id: String,
        agent_name: String,
        old_status: String,
        new_status: String,
    ) -> Result<(), AppError> {
        let message = NotificationMessage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            title: "智能体状态变更".to_string(),
            content: format!("智能体 '{}' 状态从 '{}' 变更为 '{}'", agent_name, old_status, new_status),
            notification_type: "agent_status_changed".to_string(),
            priority: "low".to_string(),
            metadata: Some(serde_json::json!({
                "agent_id": agent_id,
                "agent_name": agent_name,
                "old_status": old_status,
                "new_status": new_status
            })),
            created_at: chrono::Utc::now().timestamp_millis(),
            read: false,
        };

        self.send_notification(message).await
    }

    /// 发送系统通知
    pub async fn send_system_notification(
        &self,
        user_id: String,
        title: String,
        content: String,
        priority: String,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let message = NotificationMessage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            title,
            content,
            notification_type: "system_notification".to_string(),
            priority,
            metadata,
            created_at: chrono::Utc::now().timestamp_millis(),
            read: false,
        };

        self.send_notification(message).await
    }
}

/// 通知处理循环
pub async fn notification_processing_loop(
    mut receiver: mpsc::Receiver<NotificationMessage>,
) -> Result<(), AppError> {
    while let Some(message) = receiver.recv().await {
        let message_id = message.id.clone();
        // 处理通知消息
        match process_notification(message).await {
            Ok(_) => {
                log::info!("通知处理成功: {}", message_id);
            }
            Err(e) => {
                log::error!("通知处理失败: {} - {}", message_id, e);
            }
        }
    }

    Ok(())
}

/// 处理单个通知
async fn process_notification(message: NotificationMessage) -> Result<(), AppError> {
    // 这里可以实现通知的持久化存储
    // 例如保存到数据库、发送邮件、推送消息等
    
    log::info!(
        "处理通知: [{}] {} - {}",
        message.notification_type,
        message.title,
        message.content
    );

    // 根据通知类型进行不同的处理
    match message.notification_type.as_str() {
        "task_assigned" => {
            // 处理任务分配通知
            log::info!("任务分配通知: {}", message.content);
        }
        "task_completed" => {
            // 处理任务完成通知
            log::info!("任务完成通知: {}", message.content);
        }
        "task_failed" => {
            // 处理任务失败通知
            log::warn!("任务失败通知: {}", message.content);
        }
        "agent_status_changed" => {
            // 处理智能体状态变更通知
            log::info!("智能体状态变更通知: {}", message.content);
        }
        "system_notification" => {
            // 处理系统通知
            log::info!("系统通知: {}", message.content);
        }
        _ => {
            log::warn!("未知通知类型: {}", message.notification_type);
        }
    }

    // 这里可以添加更多通知处理逻辑
    // 例如：
    // - 保存到数据库
    // - 发送邮件
    // - 推送WebSocket消息
    // - 集成第三方通知服务

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notification_worker() {
        let (worker, mut receiver) = NotificationWorker::new();

        // 测试发送通知
        let message = NotificationMessage {
            id: "test-id".to_string(),
            user_id: "test-user".to_string(),
            title: "测试通知".to_string(),
            content: "这是一个测试通知".to_string(),
            notification_type: "test".to_string(),
            priority: "low".to_string(),
            metadata: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            read: false,
        };

        worker.send_notification(message.clone()).await.unwrap();

        // 验证通知被接收
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.id, message.id);
        assert_eq!(received.title, message.title);
    }

    #[tokio::test]
    async fn test_task_assigned_notification() {
        let (worker, _) = NotificationWorker::new();

        let result = worker
            .send_task_assigned_notification(
                "user-123".to_string(),
                "task-456".to_string(),
                "测试任务".to_string(),
                "测试智能体".to_string(),
            )
            .await;

        assert!(result.is_ok());
    }
}
