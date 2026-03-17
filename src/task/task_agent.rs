use actix::prelude::*;
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent,
};
use tracing::{debug, error};

use crate::channel::actor_messages::ResultMessage;
use crate::chat::model::MessageContent;
use crate::chat::openai_actor::{CallOpenAI, ChatAgentError, OpenAIProxyActor};
use crate::task::dag_orchestrator::{
    BeginTaskReview, CompleteTaskReview, DagOrchestrActor, FinalizeTaskDecision,
    QueryLatestReviewingTaskByUser, QueryTaskById, SubmitTask,
};
use crate::task::model::{MessageClassificationResponse, TaskInfoResponse};
use crate::utils::json_util::clean_json_string;
use crate::workspace::workspace_actor::WorkspaceManageActor;

#[derive(Debug, Clone, serde::Deserialize)]
struct TaskReviewResponse {
    approved: bool,
    review_result: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct UserDecisionResponse {
    has_decision: bool,
    approved: bool,
    reason: String,
}

/// TaskAgent Actor结构体
/// 处理意图（指令）分析
pub struct TaskAgent {
    // 工作区
    workspaces: Addr<WorkspaceManageActor>,
    dag_orchestrator: Addr<DagOrchestrActor>,
    open_aiproxy_actor: Addr<OpenAIProxyActor>,
    prompt: String,
}

impl Actor for TaskAgent {
    type Context = Context<Self>;
}

impl TaskAgent {
    pub fn new(
        open_aiproxy_actor: Addr<OpenAIProxyActor>,
        dag_orchestrator: Addr<DagOrchestrActor>,
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
        user_name: String,
        content: String,
    ) -> Result<MessageClassificationResponse, ChatAgentError> {
        match MessageClassificationResponse::from_json(clean_json_string(&content)) {
            Ok(message_classification_response) => {
                if message_classification_response.has_tasks() {
                    for task in message_classification_response.tasks.as_ref().unwrap() {
                        // 任务上传
                        match self
                            .dag_orchestrator
                            .send(SubmitTask {
                                user_name: user_name.clone(),
                                task: task.clone(),
                            })
                            .await
                        {
                            Ok(_) => debug!("任务提交成功: {:?}", task),
                            Err(e) => error!("任务提交失败: {}, 任务内容: {:?}", e, task),
                        }
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

    async fn try_finalize_review_from_user_reply(
        &self,
        user_name: String,
        user_content: MessageContent,
    ) -> Result<bool, ChatAgentError> {
        let maybe_pending = self
            .dag_orchestrator
            .send(QueryLatestReviewingTaskByUser { user_name })
            .await
            .map_err(ChatAgentError::from)?
            .map_err(|e| ChatAgentError::QueryError(format!("查询待审阅任务失败: {}", e)))?;

        let Some(pending) = maybe_pending else {
            return Ok(false);
        };

        let user_prompt = format!(
            "当前存在一个待用户决策的任务。\n任务ID: {}\n任务名称: {}\n任务描述: {}\n审阅意见: {}\n\n用户最新回复: {}\n\n请判断用户这句话是否明确表达了“通过任务/确认完成”或“驳回任务/确认未完成”。必须只返回 JSON，格式为 {{\"has_decision\":true/false,\"approved\":true/false,\"reason\":\"中文原因\"}}。",
            pending.task_id,
            pending.task_name,
            pending.task_description,
            pending.review_result,
            user_content.to_string()
        );

        let decision = match self
            .open_aiproxy_actor
            .send(CallOpenAI {
                chat_completion_request_message: vec![
                    ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                        content: ChatCompletionRequestSystemMessageContent::Text(
                            "你是任务最终决策代理，负责判断用户是否明确确认任务完成或明确驳回任务。只返回 JSON。".to_string(),
                        ),
                        name: None,
                    }),
                    ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                        name: Some("task_final_decider".to_string()),
                        content: ChatCompletionRequestUserMessageContent::Text(user_prompt),
                    }),
                ],
                tools: None,
                tool_choice: None,
                provider: None,
                model: None,
            })
            .await
            .map_err(ChatAgentError::from)?
        {
            Ok(text) => serde_json::from_str::<UserDecisionResponse>(clean_json_string(&text))
                .unwrap_or(UserDecisionResponse {
                    has_decision: false,
                    approved: false,
                    reason: "模型未能稳定解析用户决策".to_string(),
                }),
            Err(_) => UserDecisionResponse {
                has_decision: false,
                approved: false,
                reason: "模型决策失败".to_string(),
            },
        };

        if !decision.has_decision {
            return Ok(false);
        }

        self.dag_orchestrator
            .send(FinalizeTaskDecision {
                task_id: pending.task_id,
                approved: decision.approved,
                decision_reason: decision.reason,
            })
            .await
            .map_err(ChatAgentError::from)?
            .map_err(|e| ChatAgentError::LogicError(format!("更新最终任务状态失败: {}", e)))?;

        Ok(true)
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
    pub user_name: String,
}

/// 简化的异步Handler实现
impl Handler<OtherMessage> for TaskAgent {
    type Result = ResponseFuture<Result<MessageClassificationResponse, ChatAgentError>>;

    fn handle(&mut self, msg: OtherMessage, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();

        Box::pin(async move {
            if this
                .try_finalize_review_from_user_reply(msg.user_name.clone(), msg.user_content.clone())
                .await?
            {
                return Ok(MessageClassificationResponse::default());
            }

            // 1. 获取长期记忆
            let long_term_memory = msg.long_term_memory;
            // 2. 获取短期记忆
            let short_term_memory = msg.short_term_memory;
            // 3. 获取用户内容
            let user_content: MessageContent = msg.user_content;
            let user_name = msg.user_name;

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
                            name: Some(user_name.clone()),
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
            this.handle_classification(user_name, response_text).await
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

#[derive(Message, Clone)]
#[rtype(result = "Result<(), ChatAgentError>")]
pub struct ReviewSubmittedTask {
    pub task_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub selected_tool_id: Option<String>,
    pub interpreted_output: Option<String>,
    pub raw_output: Option<String>,
    pub failure_reason: Option<String>,
    pub executed: bool,
    pub should_retry: bool,
}

impl Handler<ReviewSubmittedTask> for TaskAgent {
    type Result = ResponseFuture<Result<(), ChatAgentError>>;

    fn handle(&mut self, msg: ReviewSubmittedTask, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();

        Box::pin(async move {
            this.dag_orchestrator
                .send(BeginTaskReview { task_id: msg.task_id })
                .await
                .map_err(ChatAgentError::from)?
                .map_err(|e| ChatAgentError::LogicError(format!("进入审阅状态失败: {}", e)))?;

            let task: TaskInfoResponse = this
                .dag_orchestrator
                .send(QueryTaskById(msg.task_id))
                .await
                .map_err(ChatAgentError::from)?
                .map_err(|e| ChatAgentError::QueryError(format!("查询任务失败: {}", e)))?;

            let tool_id = msg
                .selected_tool_id
                .clone()
                .unwrap_or_else(|| "unknown_tool".to_string());
            let interpreted_output = msg.interpreted_output.clone().unwrap_or_default();
            let raw_output = msg.raw_output.clone().unwrap_or_default();
            let failure_reason = msg.failure_reason.clone().unwrap_or_default();

            let system_prompt = "你是任务审阅代理。你要审阅 Agent 已提交的执行结果是否满足任务要求。必须只返回 JSON，格式为 {\"approved\": true/false, \"review_result\": \"中文审阅结论\"}。review_result 需要明确说明依据、结论，以及是否建议进入完成态。".to_string();
            let user_prompt = format!(
                "任务ID: {}\n任务名称: {}\n任务描述: {}\n工具ID: {}\n执行成功: {}\n已执行: {}\n应重试: {}\n解释结果:\n{}\n\n原始输出:\n{}\n\n失败原因:\n{}",
                msg.task_id,
                task.name,
                task.description,
                tool_id,
                msg.failure_reason.is_none(),
                msg.executed,
                msg.should_retry,
                interpreted_output,
                raw_output,
                failure_reason
            );

            let review = match this
                .open_aiproxy_actor
                .send(CallOpenAI {
                    chat_completion_request_message: vec![
                        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(system_prompt),
                            name: None,
                        }),
                        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                            name: Some("task_reviewer".to_string()),
                            content: ChatCompletionRequestUserMessageContent::Text(user_prompt),
                        }),
                    ],
                    tools: None,
                    tool_choice: None,
                    provider: None,
                    model: None,
                })
                .await
                .map_err(ChatAgentError::from)?
            {
                Ok(text) => serde_json::from_str::<TaskReviewResponse>(clean_json_string(&text))
                    .unwrap_or_else(|_| TaskReviewResponse {
                        approved: msg.executed && !msg.should_retry,
                        review_result: if !interpreted_output.trim().is_empty() {
                            interpreted_output.clone()
                        } else if !raw_output.trim().is_empty() {
                            format!("已执行工具 {}，输出如下：{}", tool_id, raw_output)
                        } else {
                            "模型审阅解析失败，回退为执行成功默认通过。".to_string()
                        },
                    }),
                Err(_) => TaskReviewResponse {
                    approved: msg.executed && !msg.should_retry,
                    review_result: if !interpreted_output.trim().is_empty() {
                        interpreted_output.clone()
                    } else if !raw_output.trim().is_empty() {
                        format!("已执行工具 {}，输出如下：{}", tool_id, raw_output)
                    } else {
                        "模型审阅失败，且无可用执行输出。".to_string()
                    },
                },
            };

            // 对外展示与持久化以 Agent 实际执行结果为准，不使用审阅建议文案。
            let execution_result = if !interpreted_output.trim().is_empty() {
                interpreted_output.clone()
            } else if !raw_output.trim().is_empty() {
                format!("已执行工具 {}，输出如下：{}", tool_id, raw_output)
            } else if !failure_reason.trim().is_empty() {
                format!("执行失败，原因：{}", failure_reason)
            } else {
                "无可用执行输出。".to_string()
            };

            this.dag_orchestrator
                .send(CompleteTaskReview {
                    task_id: msg.task_id,
                    agent_id: msg.agent_id,
                    approved: review.approved,
                    review_result: execution_result,
                })
                .await
                .map_err(ChatAgentError::from)?
                .map_err(|e| ChatAgentError::LogicError(format!("写入审阅结果失败: {}", e)))?;

            Ok(())
        })
    }
}
