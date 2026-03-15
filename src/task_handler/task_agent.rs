use actix::prelude::*;
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent,
};
use tracing::{debug, error};

use crate::chat::actor_messages::ResultMessage;
use crate::chat::model::MessageContent;
use crate::chat::openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor};
use crate::task_handler::actor_task::{DagOrchestrator, SubmitTask};
use crate::task_handler::task_model::MessageClassificationResponse;
use crate::utils::json_util::clean_json_string;
use crate::workspace::workspace_actor::WorkspaceManageActor;

/// TaskAgent Actor结构体
/// 处理意图（指令）分析
pub struct TaskAgent {
    // 工作区
    workspaces: Addr<WorkspaceManageActor>,
    dag_orchestrator: Addr<DagOrchestrator>,
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    prompt: String,
}

impl Actor for TaskAgent {
    type Context = Context<Self>;
}

impl TaskAgent {
    pub fn new(
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        dag_orchestrator: Addr<DagOrchestrator>,
        workspaces: Addr<WorkspaceManageActor>,
        prompt: String,
    ) -> Self {
        Self {
            open_aiproxy_actor,
            dag_orchestrator,
            workspaces,
            prompt,
        }
    }

    // 提示词构建
    fn build_prompt(
        &self,
        long_term_memory: &str,
        short_term_memory: Vec<ResultMessage>,
    ) -> String {
        self.prompt
            .clone()
            .replace("{memory_content}", long_term_memory)
            .replace(
                "{momory_content_short}",
                &format!("{:#?}", short_term_memory),
            )
    }

    /// 处理分类响应
    async fn handle_classification(
        &self,
        content: String,
    ) -> Result<MessageClassificationResponse, ChatAgentError> {
        match MessageClassificationResponse::from_json(clean_json_string(&content)) {
            Ok(message_classification_response) => {
                if message_classification_response.has_tasks() {
                    for task in message_classification_response.tasks.as_ref().unwrap() {
                        // 任务上传
                        let _ = self
                            .dag_orchestrator
                            .send(SubmitTask { task: task.clone() })
                            .await;
                    }
                }
                Ok(message_classification_response)
            }
            Err(e) => {
                error!("解析失败: {}, 原始内容: {}", e, content);
                Err(ChatAgentError::SerializationError(e))
            }
        }
    }
}

// 分类
#[derive(Message)]
#[rtype(result = "Result<MessageClassificationResponse, ChatAgentError>")]
pub struct OtherMessage {
    // 长期记忆
    pub long_term_memory: String,
    // 短期记忆
    pub short_term_memory: Vec<ResultMessage>,
    // 用户内容
    pub user_content: MessageContent,
}

/// 简化的异步Handler实现
impl Handler<OtherMessage> for TaskAgent {
    type Result = ResponseFuture<Result<MessageClassificationResponse, ChatAgentError>>;

    fn handle(&mut self, msg: OtherMessage, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();

        Box::pin(async move {
            // 1. 获取长期记忆
            let long_term_memory = msg.long_term_memory;
            // 2. 获取短期记忆
            let short_term_memory = msg.short_term_memory;
            // 4. 获取用户内容
            let user_content: MessageContent = msg.user_content;

            // 提示词构建
            let prompt = this.build_prompt(&long_term_memory, short_term_memory);

            // 4. 发送请求
            let response_text = this
                .open_aiproxy_actor
                .send(CallOpenAI {
                    chat_completion_request_message: vec![
                        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(prompt),
                            name: None,
                        }),
                        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                            name: None,
                            content: ChatCompletionRequestUserMessageContent::Text(
                                user_content.to_string(),
                            ),
                        }),
                    ],
                    tools: None,
                    tool_choice: None,
                    provider: None,
                    model: None,
                })
                .await
                .map_err(ChatAgentError::from)??;
            this.handle_classification(response_text).await
        })
    }
}

impl Clone for TaskAgent {
    fn clone(&self) -> Self {
        Self {
            open_aiproxy_actor: self.open_aiproxy_actor.clone(),
            dag_orchestrator: self.dag_orchestrator.clone(),
            workspaces: self.workspaces.clone(),
            prompt: self.prompt.clone(),
        }
    }
}
