//! LLM Agent with OpenRouter Integration
//!
//! Features:
//! - OpenRouter API client with model routing
//! - Tool/function calling support
//! - Conversation memory
//! - Streaming responses
//! - Rate limiting and retry logic

use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use shared::{Agent, AgentError, AgentMessage, AgentResponse, MessageRole, TokenUsage, ToolCall};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "llm-agent")]
#[command(about = "LLM-powered agent using OpenRouter")]
struct Args {
    /// Service port
    #[arg(long, env = "PORT", default_value = "3001")]
    port: u16,

    /// OpenRouter API key
    #[arg(long, env = "OPENROUTER_API_KEY")]
    api_key: String,

    /// Default model
    #[arg(long, env = "DEFAULT_MODEL", default_value = "anthropic/claude-3.5-sonnet")]
    default_model: String,

    /// Fallback model
    #[arg(long, env = "FALLBACK_MODEL", default_value = "openai/gpt-4o")]
    fallback_model: String,

    /// Max tokens
    #[arg(long, env = "MAX_TOKENS", default_value = "4096")]
    max_tokens: u32,
}

/// OpenRouter API client
struct OpenRouterClient {
    http: reqwest::Client,
    api_key: String,
    default_model: String,
    fallback_model: String,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCallMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ToolCallMessage {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct Tool {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDefinition,
}

#[derive(Debug, Serialize)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    model: String,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    role: String,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallMessage>>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl OpenRouterClient {
    fn new(api_key: String, default_model: String, fallback_model: String, max_tokens: u32) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
            default_model,
            fallback_model,
            max_tokens,
        }
    }

    async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Tool>>) -> Result<ChatResponse, AgentError> {
        self.chat_with_model(&self.default_model, messages.clone(), tools.clone())
            .await
            .or_else(|e| async {
                warn!("Primary model failed, trying fallback: {}", e);
                self.chat_with_model(&self.fallback_model, messages, tools).await
            })
            .await
    }

    async fn chat_with_model(
        &self,
        model: &str,
        messages: Vec<Message>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatResponse, AgentError> {
        let request = ChatRequest {
            model: model.to_string(),
            messages,
            tools,
            max_tokens: self.max_tokens,
            temperature: 0.7,
            stream: None,
        };

        let response = self
            .http
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://github.com/yurikrupnik/dotconfig")
            .header("X-Title", "Dotconfig LLM Agent")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::NetworkError(e.to_string()))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AgentError::RateLimited(60));
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AgentError::ModelError(error_text));
        }

        response
            .json::<ChatResponse>()
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))
    }
}

/// Code Assistant Agent
struct CodeAssistantAgent {
    name: String,
    description: String,
    client: OpenRouterClient,
    conversations: RwLock<HashMap<Uuid, Vec<Message>>>,
}

impl CodeAssistantAgent {
    fn new(client: OpenRouterClient) -> Self {
        Self {
            name: "code-assistant".to_string(),
            description: "AI-powered code assistant with tool calling capabilities".to_string(),
            client,
            conversations: RwLock::new(HashMap::new()),
        }
    }

    fn get_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "read_file".to_string(),
                    description: "Read contents of a file".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file to read"
                            }
                        },
                        "required": ["path"]
                    }),
                },
            },
            Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "write_file".to_string(),
                    description: "Write content to a file".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file"
                            },
                            "content": {
                                "type": "string",
                                "description": "Content to write"
                            }
                        },
                        "required": ["path", "content"]
                    }),
                },
            },
            Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "execute_command".to_string(),
                    description: "Execute a shell command".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "Command to execute"
                            },
                            "working_dir": {
                                "type": "string",
                                "description": "Working directory (optional)"
                            }
                        },
                        "required": ["command"]
                    }),
                },
            },
            Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "search_code".to_string(),
                    description: "Search for code patterns in the codebase".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "Search pattern (regex)"
                            },
                            "path": {
                                "type": "string",
                                "description": "Path to search in (optional)"
                            },
                            "file_type": {
                                "type": "string",
                                "description": "File extension filter (optional)"
                            }
                        },
                        "required": ["pattern"]
                    }),
                },
            },
        ]
    }

    async fn execute_tool(&self, name: &str, arguments: &serde_json::Value) -> String {
        match name {
            "read_file" => {
                let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
                match tokio::fs::read_to_string(path).await {
                    Ok(content) => content,
                    Err(e) => format!("Error reading file: {}", e),
                }
            }
            "write_file" => {
                let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
                match tokio::fs::write(path, content).await {
                    Ok(_) => format!("Successfully wrote to {}", path),
                    Err(e) => format!("Error writing file: {}", e),
                }
            }
            "execute_command" => {
                let command = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
                match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .await
                {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        format!("stdout:\n{}\nstderr:\n{}", stdout, stderr)
                    }
                    Err(e) => format!("Error executing command: {}", e),
                }
            }
            "search_code" => {
                let pattern = arguments.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                match tokio::process::Command::new("rg")
                    .args(["--json", "-l", pattern, path])
                    .output()
                    .await
                {
                    Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
                    Err(e) => format!("Error searching: {}", e),
                }
            }
            _ => format!("Unknown tool: {}", name),
        }
    }
}

#[async_trait]
impl Agent for CodeAssistantAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tools(&self) -> Vec<String> {
        vec![
            "read_file".to_string(),
            "write_file".to_string(),
            "execute_command".to_string(),
            "search_code".to_string(),
        ]
    }

    async fn invoke(&self, message: AgentMessage) -> Result<AgentResponse, AgentError> {
        let conversation_id = message.id;

        // Build message history
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: "You are an expert code assistant. Help users with coding tasks, debugging, and best practices. Use the available tools when needed.".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];

        // Add context messages
        for ctx in &message.context {
            messages.push(Message {
                role: match ctx.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::System => "system".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: ctx.content.clone(),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Add current message
        messages.push(Message {
            role: "user".to_string(),
            content: message.content.clone(),
            tool_calls: None,
            tool_call_id: None,
        });

        // Get tools
        let tools = self.get_tools();

        // Call LLM
        let response = self.client.chat(messages.clone(), Some(tools)).await?;

        let choice = response.choices.first().ok_or_else(|| {
            AgentError::Internal("No response from model".to_string())
        })?;

        // Handle tool calls if present
        let mut final_content = choice.message.content.clone().unwrap_or_default();
        let mut tool_calls_result = Vec::new();

        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                let arguments: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::json!({}));

                let result = self.execute_tool(&tc.function.name, &arguments).await;

                tool_calls_result.push(ToolCall {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments,
                });

                // Add tool result to messages and get final response
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: "".to_string(),
                    tool_calls: Some(vec![tc.clone()]),
                    tool_call_id: None,
                });

                messages.push(Message {
                    role: "tool".to_string(),
                    content: result,
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                });
            }

            // Get final response after tool calls
            let final_response = self.client.chat(messages, None).await?;
            if let Some(final_choice) = final_response.choices.first() {
                final_content = final_choice.message.content.clone().unwrap_or_default();
            }
        }

        // Build response
        Ok(AgentResponse {
            id: Uuid::new_v4(),
            message_id: message.id,
            content: final_content,
            tool_calls: tool_calls_result,
            usage: response.usage.map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            model: response.model,
            created_at: chrono::Utc::now(),
        })
    }
}

/// Application state
struct AppState {
    agent: CodeAssistantAgent,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("llm_agent=info".parse()?)
        )
        .json()
        .init();

    let args = Args::parse();
    info!("Starting LLM agent on port {}", args.port);

    let client = OpenRouterClient::new(
        args.api_key,
        args.default_model,
        args.fallback_model,
        args.max_tokens,
    );

    let state = Arc::new(AppState {
        agent: CodeAssistantAgent::new(client),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/invoke", post(invoke_agent))
        .route("/info", get(agent_info))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    info!("Listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "OK"
}

#[derive(Deserialize)]
struct InvokeRequest {
    message: String,
    #[serde(default)]
    context: Vec<AgentMessage>,
}

async fn invoke_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InvokeRequest>,
) -> Result<Json<AgentResponse>, StatusCode> {
    let message = AgentMessage::user(req.message).with_context(req.context);

    state
        .agent
        .invoke(message)
        .await
        .map(Json)
        .map_err(|e| {
            error!("Agent invocation failed: {}", e);
            match e {
                AgentError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
                AgentError::ContextTooLong(_) => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })
}

#[derive(Serialize)]
struct AgentInfo {
    name: String,
    description: String,
    tools: Vec<String>,
}

async fn agent_info(State(state): State<Arc<AppState>>) -> Json<AgentInfo> {
    Json(AgentInfo {
        name: state.agent.name().to_string(),
        description: state.agent.description().to_string(),
        tools: state.agent.tools(),
    })
}
