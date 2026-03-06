pub mod user;
pub mod task;
pub mod agent;
pub mod workspace;
pub mod message;
pub mod workflow;
pub mod channel;
pub mod chat;

// 重新导出常用类型
pub use user::{User, UserResponse, CreateUserRequest, LoginRequest, AuthResponse};
pub use task::{Task, TaskResponse, CreateTaskRequest, TaskStatus, TaskPriority};
pub use agent::{Agent, AgentResponse, RegisterAgentRequest, AgentStatus};
pub use workspace::{Workspace, WorkspaceResponse, CreateWorkspaceRequest, PermissionLevel};
pub use message::{AgentMessage, TaskMessage, UserMessage, SystemBroadcast, SendMessageRequest, MessageResponse, MessageListResponse};
pub use workflow::{
    Workflow, WorkflowExecution, WorkflowResponse, WorkflowExecutionResponse,
    CreateWorkflowRequest, ExecuteWorkflowRequest,
};
pub use channel::{ChannelConfig, ChannelUser, ChannelType, CreateChannelConfigRequest, UpdateChannelConfigRequest};
pub use chat::{ChatSession, ChatMessage, MessageRole, CreateChatSessionRequest, SendChatMessageRequest, LLMConfig};
