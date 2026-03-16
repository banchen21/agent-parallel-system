use thiserror::Error;

/// 错误类型
#[derive(Debug, Error)]
pub enum AgentError {
    
    #[error("数据库操作失败: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Actor 通信失败 (可能已宕机或超时): {0}")]
    MailboxError(#[from] actix::MailboxError), // 自动处理 actix 的 send 报错

    #[error("文件系统/IO 操作失败: {0}")]
    IoError(#[from] std::io::Error), // 如果你的工作区涉及本地文件夹的创建删除，必须加这个

    // === 2. 业务逻辑错误 (400 / 404 / 409) ===
    #[error("未找到对应的工作区: {0}")]
    NotFound(String), // 查询或删除时找不到对应数据

    #[error("该工作区已存在: {0}")]
    AlreadyExists(String), // 创建时发生名称冲突

    #[error("操作失败: {0}")]
    Message(String), // 通用的其他业务报错
}

