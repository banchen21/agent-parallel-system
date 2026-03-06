use serde::{Deserialize, Serialize};
use reqwest::Client;

use crate::core::errors::AppError;

/// OpenAI 兼容的消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI 兼容的请求
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    pub temperature: f32,
    pub max_tokens: i32,
}

/// OpenAI 兼容的响应
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionChoice {
    pub message: ChatCompletionMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// LLM 客户端
pub struct LLMClient {
    http_client: Client,
    api_endpoint: String,
    api_key: Option<String>,
    model: String,
}

impl LLMClient {
    pub fn new(api_endpoint: String, api_key: Option<String>, model: String) -> Self {
        Self {
            http_client: Client::new(),
            api_endpoint,
            api_key,
            model,
        }
    }

    /// 调用 LLM API
    pub async fn chat_completion(
        &self,
        messages: Vec<ChatCompletionMessage>,
        temperature: f32,
        max_tokens: i32,
    ) -> Result<ChatCompletionResponse, AppError> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
        };

        let mut req = self.http_client
            .post(format!("{}/chat/completions", self.api_endpoint))
            .json(&request);

        // 如果有 API key，添加到请求头
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req
            .send()
            .await
            .map_err(|_e| AppError::InternalServerError)?;

        if !response.status().is_success() {
            return Err(AppError::InternalServerError);
        }

        let completion = response
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|_e| AppError::InternalServerError)?;

        Ok(completion)
    }

    /// 获取单个回复
    pub async fn get_response(
        &self,
        messages: Vec<ChatCompletionMessage>,
        temperature: f32,
        max_tokens: i32,
    ) -> Result<(String, Option<i32>), AppError> {
        let response = self.chat_completion(messages, temperature, max_tokens).await?;

        if response.choices.is_empty() {
            return Err(AppError::InternalServerError);
        }

        let choice = &response.choices[0];
        let tokens_used = response.usage.map(|u| u.total_tokens);

        Ok((choice.message.content.clone(), tokens_used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_client_creation() {
        let client = LLMClient::new(
            "http://localhost:8000".to_string(),
            Some("test-key".to_string()),
            "gpt-3.5-turbo".to_string(),
        );
        assert_eq!(client.model, "gpt-3.5-turbo");
    }
}
