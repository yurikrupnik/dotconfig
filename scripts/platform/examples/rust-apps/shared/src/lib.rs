//! Shared types and traits for platform examples

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Skill trait - implement this for CPU-intensive workloads
#[async_trait]
pub trait Skill: Send + Sync {
    /// Skill name for registration
    fn name(&self) -> &str;

    /// Skill description
    fn description(&self) -> &str;

    /// Execute the skill with input data
    async fn execute(&self, input: SkillInput) -> Result<SkillOutput, SkillError>;

    /// Health check
    async fn health(&self) -> bool {
        true
    }
}

/// Input for skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    pub id: Uuid,
    pub data: serde_json::Value,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

impl SkillInput {
    pub fn new(data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Output from skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    pub id: Uuid,
    pub input_id: Uuid,
    pub result: serde_json::Value,
    pub metadata: HashMap<String, String>,
    pub duration_ms: u64,
    pub created_at: DateTime<Utc>,
}

impl SkillOutput {
    pub fn new(input_id: Uuid, result: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            input_id,
            result,
            metadata: HashMap::new(),
            duration_ms,
            created_at: Utc::now(),
        }
    }
}

/// Skill execution errors
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout after {0}ms")]
    Timeout(u64),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Agent trait - implement this for LLM-powered agents
#[async_trait]
pub trait Agent: Send + Sync {
    /// Agent name
    fn name(&self) -> &str;

    /// Agent description
    fn description(&self) -> &str;

    /// Available tools/skills
    fn tools(&self) -> Vec<String>;

    /// Process a message and return response
    async fn invoke(&self, message: AgentMessage) -> Result<AgentResponse, AgentError>;

    /// Stream responses (for long-running operations)
    async fn invoke_stream(
        &self,
        message: AgentMessage,
    ) -> Result<AgentResponseStream, AgentError> {
        // Default implementation: single response
        let response = self.invoke(message).await?;
        Ok(AgentResponseStream::Single(response))
    }
}

/// Message to an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: Uuid,
    pub content: String,
    pub role: MessageRole,
    pub context: Vec<AgentMessage>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

impl AgentMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            role: MessageRole::User,
            context: Vec::new(),
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    pub fn with_context(mut self, context: Vec<AgentMessage>) -> Self {
        self.context = context;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Response from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: Uuid,
    pub message_id: Uuid,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
    pub model: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub enum AgentResponseStream {
    Single(AgentResponse),
    // Future: Stream implementation
}

/// Agent errors
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Model error: {0}")]
    ModelError(String),

    #[error("Rate limited: retry after {0}s")]
    RateLimited(u64),

    #[error("Context too long: {0} tokens")]
    ContextTooLong(u32),

    #[error("Tool execution failed: {0}")]
    ToolError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Task for queue-based processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub priority: u8,
    pub retries: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub scheduled_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new(task_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_type: task_type.into(),
            payload,
            priority: 5,
            retries: 0,
            max_retries: 3,
            created_at: Utc::now(),
            scheduled_at: None,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: Uuid,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Retrying,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_input() {
        let input = SkillInput::new(serde_json::json!({"key": "value"}))
            .with_metadata("source", "test");

        assert_eq!(input.metadata.get("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_agent_message() {
        let msg = AgentMessage::user("Hello, world!");
        assert_eq!(msg.content, "Hello, world!");
        assert!(matches!(msg.role, MessageRole::User));
    }

    #[test]
    fn test_task() {
        let task = Task::new("process", serde_json::json!({"file": "test.txt"}))
            .with_priority(10);

        assert_eq!(task.priority, 10);
        assert_eq!(task.max_retries, 3);
    }
}
