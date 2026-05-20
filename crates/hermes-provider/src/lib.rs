use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderProfileSummary {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub api_mode: &'static str,
    pub display_name: &'static str,
    pub env_vars: &'static [&'static str],
    pub base_url: &'static str,
    pub auth_type: &'static str,
    pub supports_health_check: bool,
    pub fallback_model_count: usize,
    pub default_max_tokens: Option<i64>,
    pub fixed_temperature: Option<&'static str>,
    pub default_header_keys: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

pub trait ChatProvider {
    fn chat(&mut self, request: Value) -> ProviderResponse;
}

#[derive(Debug, Clone)]
pub struct FakeProvider {
    responses: VecDeque<ProviderResponse>,
    requests: Vec<Value>,
}

impl FakeProvider {
    pub fn new(responses: impl IntoIterator<Item = ProviderResponse>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: Vec::new(),
        }
    }

    pub fn requests(&self) -> &[Value] {
        &self.requests
    }
}

impl ChatProvider for FakeProvider {
    fn chat(&mut self, request: Value) -> ProviderResponse {
        self.requests.push(request);
        self.responses.pop_front().unwrap_or(ProviderResponse {
            content: Some(String::new()),
            tool_calls: Vec::new(),
        })
    }
}

pub fn provider_profiles() -> &'static [ProviderProfileSummary] {
    PROVIDER_PROFILES
}

pub fn provider_by_name(name: &str) -> Option<&'static ProviderProfileSummary> {
    PROVIDER_PROFILES
        .iter()
        .find(|profile| profile.name == name)
}

pub fn resolve_provider(name_or_alias: &str) -> Option<&'static ProviderProfileSummary> {
    PROVIDER_PROFILES
        .iter()
        .find(|profile| profile.name == name_or_alias || profile.aliases.contains(&name_or_alias))
}

pub fn fake_chat_completions_request() -> Value {
    json!({
        "max_tokens": 123,
        "messages": [
            {"content": "system", "role": "system"},
            {"content": "hello", "role": "user"}
        ],
        "model": "fake/model",
        "temperature": 0.2,
        "timeout": 30,
        "tools": [
            {
                "function": {
                    "description": "Remember facts.",
                    "name": "memory",
                    "parameters": {
                        "properties": {
                            "action": {"type": "string"}
                        },
                        "required": ["action"],
                        "type": "object"
                    }
                },
                "type": "function"
            }
        ]
    })
}

pub fn chat_completions_strips_codex_leaks_request() -> Value {
    json!({
        "messages": [
            {
                "content": "ok",
                "role": "assistant",
                "tool_calls": [
                    {
                        "function": {
                            "arguments": "{}",
                            "name": "terminal"
                        },
                        "id": "call_1",
                        "type": "function"
                    }
                ]
            }
        ],
        "model": "gpt-4o",
        "timeout": 10
    })
}

pub fn codex_responses_standard_request() -> Value {
    json!({
        "include": ["reasoning.encrypted_content"],
        "input": [
            {"content": "hello", "role": "user"}
        ],
        "instructions": "system",
        "max_output_tokens": 4096,
        "model": "gpt-5.4",
        "parallel_tool_calls": true,
        "prompt_cache_key": "session-1",
        "reasoning": {
            "effort": "low",
            "summary": "auto"
        },
        "store": false,
        "tool_choice": "auto",
        "tools": [
            {
                "description": "Remember facts.",
                "name": "memory",
                "parameters": {
                    "properties": {
                        "action": {"type": "string"}
                    },
                    "required": ["action"],
                    "type": "object"
                },
                "strict": false,
                "type": "function"
            }
        ]
    })
}

pub fn codex_responses_xai_cache_routing_request() -> Value {
    json!({
        "extra_body": {
            "caller": true,
            "prompt_cache_key": "conv-xai-1"
        },
        "extra_headers": {
            "X-Test": "1",
            "x-grok-conv-id": "conv-xai-1"
        },
        "include": [],
        "input": [
            {"content": "hello", "role": "user"}
        ],
        "instructions": "You are Hermes Agent, an intelligent AI assistant created by Nous Research. You are helpful, knowledgeable, and direct. You assist users with a wide range of tasks including answering questions, writing and editing code, analyzing information, creative work, and executing actions via your tools. You communicate clearly, admit uncertainty when appropriate, and prioritize being genuinely useful over being verbose unless otherwise directed below. Be targeted and efficient in your exploration and investigations.",
        "max_output_tokens": 2048,
        "model": "grok-3-mini",
        "reasoning": {
            "effort": "high"
        },
        "store": false,
        "tools": null
    })
}

pub fn anthropic_messages_standard_request() -> Value {
    json!({
        "max_tokens": 1024,
        "messages": [
            {"content": "hello", "role": "user"}
        ],
        "model": "claude-sonnet-4-6",
        "output_config": {
            "effort": "high"
        },
        "system": "system",
        "thinking": {
            "display": "summarized",
            "type": "adaptive"
        },
        "tool_choice": {
            "type": "any"
        },
        "tools": [
            {
                "description": "Remember facts.",
                "input_schema": {
                    "properties": {
                        "action": {"type": "string"}
                    },
                    "required": ["action"],
                    "type": "object"
                },
                "name": "memory"
            }
        ]
    })
}

pub fn normalized_transport_types_fixture() -> Value {
    json!({
        "finish_reason_map": {
            "known": map_finish_reason(Some("tool_use")),
            "none": map_finish_reason(None),
            "unknown": map_finish_reason(Some("something_new")),
        },
        "name": "normalized_transport_types",
        "response": {
            "codex_message_items": [{"id": "msg_1", "type": "message"}],
            "codex_reasoning_items": [{"id": "rs_1"}],
            "content": "answer",
            "finish_reason": "tool_calls",
            "reasoning": "I thought about it",
            "reasoning_content": "hidden chain",
            "reasoning_details": [{"thinking": "hmm", "type": "thinking"}],
        },
        "tool_call": {
            "arguments": "{\"cmd\": \"ls\"}",
            "call_id": "call_3",
            "extra_content": {"google": {"thought_signature": "SIG_ABC123"}},
            "function_arguments": "{\"cmd\": \"ls\"}",
            "function_is_self": true,
            "function_name": "terminal",
            "id": "call_3",
            "name": "terminal",
            "provider_data": {
                "call_id": "call_3",
                "extra_content": {"google": {"thought_signature": "SIG_ABC123"}},
                "response_item_id": "fc_3",
            },
            "response_item_id": "fc_3",
            "type": "function",
        },
        "usage_defaults": {
            "cached_tokens": 0,
            "completion_tokens": 0,
            "prompt_tokens": 0,
            "total_tokens": 0,
        },
    })
}

pub fn map_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("end_turn" | "stop_sequence") => "stop",
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        Some("refusal") => "content_filter",
        _ => "stop",
    }
}

pub fn model_requires_responses_api(model: &str) -> bool {
    let normalized = model
        .to_ascii_lowercase()
        .rsplit_once('/')
        .map(|(_, model)| model.to_string())
        .unwrap_or_else(|| model.to_ascii_lowercase());
    normalized.starts_with("gpt-5")
}

pub fn provider_model_requires_responses_api(model: &str, provider: Option<&str>) -> bool {
    let normalized_provider = provider.unwrap_or_default().trim().to_ascii_lowercase();
    if normalized_provider == "nous" {
        return false;
    }
    if normalized_provider == "copilot" {
        return copilot_model_requires_responses_api(model);
    }
    model_requires_responses_api(model)
}

fn copilot_model_requires_responses_api(model: &str) -> bool {
    model_requires_responses_api(model)
}

pub fn max_tokens_param(base_url: &str, value: i64) -> Value {
    if is_direct_openai_url(base_url)
        || is_azure_openai_url(base_url)
        || is_github_copilot_url(base_url)
    {
        json!({"max_completion_tokens": value})
    } else {
        json!({"max_tokens": value})
    }
}

pub fn is_direct_openai_url(base_url: &str) -> bool {
    base_url_hostname(base_url) == "api.openai.com"
}

pub fn is_azure_openai_url(base_url: &str) -> bool {
    base_url.to_ascii_lowercase().contains("openai.azure.com")
}

pub fn is_github_copilot_url(base_url: &str) -> bool {
    base_url_hostname(base_url) == "api.githubcopilot.com"
}

fn base_url_hostname(base_url: &str) -> String {
    let lower = base_url.trim().to_ascii_lowercase();
    let without_scheme = lower
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(lower.as_str());
    let authority = without_scheme.split('/').next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[derive(Debug, Default)]
pub struct CodexEventProjector {
    pending_reasoning: Vec<String>,
}

impl CodexEventProjector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project(&mut self, notification: &Value) -> Value {
        let method = get_str(notification, "method");
        let params = notification.get("params").unwrap_or(&Value::Null);
        if method != "item/completed" {
            return projection_result(Vec::new(), false, None);
        }

        let item = params.get("item").unwrap_or(&Value::Null);
        let item_type = get_str(item, "type");
        let item_id = get_str(item, "id");
        match item_type {
            "agentMessage" => self.project_agent_message(item),
            "reasoning" => {
                self.extend_reasoning(item.get("summary"));
                self.extend_reasoning(item.get("content"));
                projection_result(Vec::new(), false, None)
            }
            "commandExecution" => self.project_command(item, item_id),
            "fileChange" => self.project_file_change(item, item_id),
            "mcpToolCall" => self.project_mcp_tool_call(item, item_id),
            "dynamicToolCall" => self.project_dynamic_tool_call(item, item_id),
            "userMessage" => self.project_user_message(item),
            _ => self.project_opaque(item, item_type),
        }
    }

    fn extend_reasoning(&mut self, value: Option<&Value>) {
        if let Some(items) = value.and_then(Value::as_array) {
            self.pending_reasoning.extend(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned),
            );
        }
    }

    fn project_agent_message(&mut self, item: &Value) -> Value {
        let text = get_str(item, "text").to_string();
        let mut message = json!({"role": "assistant", "content": text});
        self.attach_pending_reasoning(&mut message);
        projection_result(
            vec![message],
            false,
            Some(get_str(item, "text").to_string()),
        )
    }

    fn project_user_message(&mut self, item: &Value) -> Value {
        let mut text_parts = Vec::new();
        if let Some(fragments) = item.get("content").and_then(Value::as_array) {
            for fragment in fragments {
                if !fragment.is_object() {
                    continue;
                }
                if get_str(fragment, "type") == "text" {
                    text_parts.push(get_str(fragment, "text").to_string());
                } else if let Some(text) = fragment.get("text") {
                    if let Some(text) = text.as_str() {
                        text_parts.push(text.to_string());
                    } else {
                        text_parts.push(text.to_string());
                    }
                }
            }
        }
        projection_result(
            vec![json!({"role": "user", "content": text_parts.join("\n")})],
            false,
            None,
        )
    }

    fn project_command(&mut self, item: &Value, item_id: &str) -> Value {
        let call_id = deterministic_call_id("exec", item_id);
        let args = json!({
            "command": get_str(item, "command"),
            "cwd": get_str(item, "cwd"),
        });
        let mut assistant = tool_call_message(&call_id, "exec_command", &python_json_sorted(&args));
        self.attach_pending_reasoning(&mut assistant);

        let mut output = get_str(item, "aggregatedOutput").to_string();
        if let Some(exit_code) = item.get("exitCode").and_then(Value::as_i64) {
            if exit_code != 0 {
                output = format!("[exit {exit_code}]\n{output}");
            }
        }
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": output,
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_file_change(&mut self, item: &Value, item_id: &str) -> Value {
        let call_id = deterministic_call_id("apply_patch", item_id);
        let mut changes = Vec::new();
        if let Some(items) = item.get("changes").and_then(Value::as_array) {
            for change in items {
                let kind = change
                    .get("kind")
                    .and_then(|kind| kind.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("update");
                changes.push(json!({
                    "kind": kind,
                    "path": get_str(change, "path"),
                }));
            }
        }
        let args = json!({"changes": changes});
        let mut assistant = tool_call_message(&call_id, "apply_patch", &python_json_sorted(&args));
        self.attach_pending_reasoning(&mut assistant);
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": format!(
                "apply_patch status={}, {} change(s)",
                value_str_or(item.get("status"), "unknown"),
                item.get("changes").and_then(Value::as_array).map_or(0, Vec::len)
            ),
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_mcp_tool_call(&mut self, item: &Value, item_id: &str) -> Value {
        let server = value_str_or(item.get("server"), "mcp");
        let tool_name = value_str_or(item.get("tool"), "unknown");
        let call_id = deterministic_call_id(&format!("mcp_{server}_{tool_name}"), item_id);
        let arguments = item
            .get("arguments")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(
                || json!({"arguments": item.get("arguments").cloned().unwrap_or(Value::Null)}),
            );
        let mut assistant = tool_call_message(
            &call_id,
            &format!("mcp.{server}.{tool_name}"),
            &python_json_sorted(&arguments),
        );
        self.attach_pending_reasoning(&mut assistant);
        let content = if let Some(error) = item.get("error").filter(|value| !value.is_null()) {
            format!(
                "[error] {}",
                truncate_chars(&python_json_in_order(error), 1000)
            )
        } else if let Some(result) = item.get("result").filter(|value| !value.is_null()) {
            truncate_chars(&python_json_in_order(result), 4000)
        } else {
            String::new()
        };
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": content,
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_dynamic_tool_call(&mut self, item: &Value, item_id: &str) -> Value {
        let tool_name = value_str_or(item.get("tool"), "unknown");
        let call_id = deterministic_call_id(&format!("dyn_{tool_name}"), item_id);
        let arguments = item
            .get("arguments")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(
                || json!({"arguments": item.get("arguments").cloned().unwrap_or(Value::Null)}),
            );
        let mut assistant = tool_call_message(&call_id, tool_name, &python_json_sorted(&arguments));
        self.attach_pending_reasoning(&mut assistant);
        let content = item
            .get("contentItems")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .map(|_| truncate_chars(&python_json_in_order(&item["contentItems"]), 4000))
            .unwrap_or_else(|| format!("success={}", python_bool(item.get("success"))));
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": content,
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_opaque(&mut self, item: &Value, item_type: &str) -> Value {
        let payload = truncate_chars(&python_json_in_order(item), 1500);
        projection_result(
            vec![json!({
                "role": "assistant",
                "content": format!("[codex {item_type}] {payload}"),
            })],
            false,
            None,
        )
    }

    fn attach_pending_reasoning(&mut self, message: &mut Value) {
        if self.pending_reasoning.is_empty() {
            return;
        }
        if let Some(object) = message.as_object_mut() {
            object.insert(
                "reasoning".to_string(),
                json!(self.pending_reasoning.join("\n")),
            );
        }
        self.pending_reasoning.clear();
    }
}

pub fn project_codex_event_notifications(notifications: &[Value]) -> Vec<Value> {
    let mut projector = CodexEventProjector::new();
    notifications
        .iter()
        .map(|notification| projector.project(notification))
        .collect()
}

fn projection_result(
    messages: Vec<Value>,
    is_tool_iteration: bool,
    final_text: Option<String>,
) -> Value {
    json!({
        "messages": messages,
        "is_tool_iteration": is_tool_iteration,
        "final_text": final_text,
    })
}

fn deterministic_call_id(item_type: &str, item_id: &str) -> String {
    if !item_id.is_empty() {
        format!("codex_{item_type}_{item_id}")
    } else {
        format!("codex_{item_type}_")
    }
}

fn tool_call_message(call_id: &str, name: &str, arguments: &str) -> Value {
    json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [
            {
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": arguments,
                },
            }
        ],
    })
}

fn get_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn value_str_or<'a>(value: Option<&'a Value>, fallback: &'a str) -> &'a str {
    value.and_then(Value::as_str).unwrap_or(fallback)
}

fn python_bool(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_bool) {
        Some(true) => "True",
        Some(false) => "False",
        None => "None",
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn python_json_sorted(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(python_json_sorted)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap(),
                        python_json_sorted(&object[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => python_json_scalar(value),
    }
}

fn python_json_in_order(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(python_json_in_order)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(object) => {
            let keys = object.keys().collect::<Vec<_>>();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap(),
                        python_json_in_order(&object[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => python_json_scalar(value),
    }
}

fn python_json_scalar(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap(),
    }
}

const PROVIDER_PROFILES: &[ProviderProfileSummary] = &[
    ProviderProfileSummary {
        name: "ai-gateway",
        aliases: &["ai_gateway", "aigateway", "vercel", "vercel-ai-gateway"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["AI_GATEWAY_API_KEY"],
        base_url: "https://ai-gateway.vercel.sh/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &["HTTP-Referer", "X-Title"],
    },
    ProviderProfileSummary {
        name: "alibaba",
        aliases: &["alibaba-cloud", "dashscope", "qwen-dashscope"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["DASHSCOPE_API_KEY"],
        base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "alibaba-coding-plan",
        aliases: &["alibaba-coding", "alibaba_coding", "dashscope-coding"],
        api_mode: "chat_completions",
        display_name: "Alibaba Cloud (Coding Plan)",
        env_vars: &[
            "ALIBABA_CODING_PLAN_API_KEY",
            "DASHSCOPE_API_KEY",
            "ALIBABA_CODING_PLAN_BASE_URL",
        ],
        base_url: "https://coding-intl.dashscope.aliyuncs.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "anthropic",
        aliases: &["claude", "claude-code", "claude-oauth"],
        api_mode: "anthropic_messages",
        display_name: "",
        env_vars: &[
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
        ],
        base_url: "https://api.anthropic.com",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "arcee",
        aliases: &["arcee-ai", "arceeai"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["ARCEEAI_API_KEY"],
        base_url: "https://api.arcee.ai/api/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "azure-foundry",
        aliases: &["azure", "azure-ai", "azure-ai-foundry"],
        api_mode: "chat_completions",
        display_name: "Azure Foundry",
        env_vars: &["AZURE_FOUNDRY_API_KEY", "AZURE_FOUNDRY_BASE_URL"],
        base_url: "",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "bedrock",
        aliases: &["amazon", "amazon-bedrock", "aws", "aws-bedrock"],
        api_mode: "bedrock_converse",
        display_name: "",
        env_vars: &[],
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com",
        auth_type: "aws_sdk",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "copilot",
        aliases: &["github", "github-copilot", "github-model", "github-models"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"],
        base_url: "https://api.githubcopilot.com",
        auth_type: "copilot",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "copilot-acp",
        aliases: &["copilot-acp-agent", "github-copilot-acp"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &[],
        base_url: "acp://copilot",
        auth_type: "external_process",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "custom",
        aliases: &[
            "llama-cpp",
            "llama.cpp",
            "llamacpp",
            "local",
            "ollama",
            "vllm",
        ],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &[],
        base_url: "",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "deepseek",
        aliases: &["deepseek-chat"],
        api_mode: "chat_completions",
        display_name: "DeepSeek",
        env_vars: &["DEEPSEEK_API_KEY"],
        base_url: "https://api.deepseek.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "gemini",
        aliases: &["google", "google-ai-studio", "google-gemini"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["GOOGLE_API_KEY", "GEMINI_API_KEY"],
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "gmi",
        aliases: &["gmi-cloud", "gmicloud"],
        api_mode: "chat_completions",
        display_name: "GMI Cloud",
        env_vars: &["GMI_API_KEY", "GMI_BASE_URL"],
        base_url: "https://api.gmi-serving.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 6,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &["User-Agent"],
    },
    ProviderProfileSummary {
        name: "google-gemini-cli",
        aliases: &["gemini-cli", "gemini-oauth"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &[],
        base_url: "cloudcode-pa://google",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "huggingface",
        aliases: &["hf", "hugging-face", "huggingface-hub"],
        api_mode: "chat_completions",
        display_name: "HuggingFace",
        env_vars: &["HF_TOKEN"],
        base_url: "https://router.huggingface.co/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "kilocode",
        aliases: &["kilo", "kilo-code", "kilo-gateway"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["KILOCODE_API_KEY"],
        base_url: "https://api.kilo.ai/api/gateway",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "kimi-coding",
        aliases: &["kimi", "kimi-for-coding", "moonshot"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["KIMI_API_KEY", "KIMI_CODING_API_KEY"],
        base_url: "https://api.moonshot.ai/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: Some(32000),
        fixed_temperature: Some("omit"),
        default_header_keys: &["User-Agent"],
    },
    ProviderProfileSummary {
        name: "kimi-coding-cn",
        aliases: &["kimi-cn", "moonshot-cn"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["KIMI_CN_API_KEY"],
        base_url: "https://api.moonshot.cn/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: Some(32000),
        fixed_temperature: Some("omit"),
        default_header_keys: &["User-Agent"],
    },
    ProviderProfileSummary {
        name: "minimax",
        aliases: &["mini-max"],
        api_mode: "anthropic_messages",
        display_name: "",
        env_vars: &["MINIMAX_API_KEY"],
        base_url: "https://api.minimax.io/anthropic",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "minimax-cn",
        aliases: &["minimax-china", "minimax_cn"],
        api_mode: "anthropic_messages",
        display_name: "",
        env_vars: &["MINIMAX_CN_API_KEY"],
        base_url: "https://api.minimaxi.com/anthropic",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "minimax-oauth",
        aliases: &["minimax-oauth-io", "minimax_oauth"],
        api_mode: "anthropic_messages",
        display_name: "MiniMax (OAuth)",
        env_vars: &[],
        base_url: "https://api.minimax.io/anthropic",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "nous",
        aliases: &["nous-portal", "nousresearch"],
        api_mode: "chat_completions",
        display_name: "Nous Research",
        env_vars: &["NOUS_API_KEY"],
        base_url: "https://inference.nousresearch.com/v1",
        auth_type: "oauth_device_code",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "novita",
        aliases: &["novita-ai", "novitaai"],
        api_mode: "chat_completions",
        display_name: "NovitaAI",
        env_vars: &["NOVITA_API_KEY", "NOVITA_BASE_URL"],
        base_url: "https://api.novita.ai/openai/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 6,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "nvidia",
        aliases: &["nvidia-nim"],
        api_mode: "chat_completions",
        display_name: "NVIDIA NIM",
        env_vars: &["NVIDIA_API_KEY"],
        base_url: "https://integrate.api.nvidia.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: Some(16384),
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "ollama-cloud",
        aliases: &["ollama_cloud"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["OLLAMA_API_KEY"],
        base_url: "https://ollama.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "openai-codex",
        aliases: &["codex", "openai_codex"],
        api_mode: "codex_responses",
        display_name: "",
        env_vars: &[],
        base_url: "https://chatgpt.com/backend-api/codex",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "opencode-go",
        aliases: &["go", "opencode-go-sub", "opencode_go"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["OPENCODE_GO_API_KEY"],
        base_url: "https://opencode.ai/zen/go/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "opencode-zen",
        aliases: &["opencode", "opencode_zen", "zen"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["OPENCODE_ZEN_API_KEY"],
        base_url: "https://opencode.ai/zen/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "openrouter",
        aliases: &["or"],
        api_mode: "chat_completions",
        display_name: "OpenRouter",
        env_vars: &["OPENROUTER_API_KEY"],
        base_url: "https://openrouter.ai/api/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 5,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "qwen-oauth",
        aliases: &["qwen", "qwen-cli", "qwen-portal"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["QWEN_API_KEY"],
        base_url: "https://portal.qwen.ai/v1",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: Some(65536),
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "stepfun",
        aliases: &["step", "stepfun-coding-plan"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["STEPFUN_API_KEY"],
        base_url: "https://api.stepfun.ai/step_plan/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "xai",
        aliases: &["grok", "x-ai", "x.ai"],
        api_mode: "codex_responses",
        display_name: "",
        env_vars: &["XAI_API_KEY"],
        base_url: "https://api.x.ai/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "xiaomi",
        aliases: &["mimo", "xiaomi-mimo"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["XIAOMI_API_KEY"],
        base_url: "https://api.xiaomimimo.com/v1",
        auth_type: "api_key",
        supports_health_check: false,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "zai",
        aliases: &["glm", "z-ai", "z.ai", "zhipu"],
        api_mode: "chat_completions",
        display_name: "Z.AI (GLM)",
        env_vars: &["GLM_API_KEY", "ZAI_API_KEY", "Z_AI_API_KEY"],
        base_url: "https://api.z.ai/api/paas/v4",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
];
