use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::collections::VecDeque;

const DEFAULT_AGENT_IDENTITY: &str = "You are Hermes Agent, an intelligent AI assistant created by Nous Research. You are helpful, knowledgeable, and direct. You assist users with a wide range of tasks including answering questions, writing and editing code, analyzing information, creative work, and executing actions via your tools. You communicate clearly, admit uncertainty when appropriate, and prioritize being genuinely useful over being verbose unless otherwise directed below. Be targeted and efficient in your exploration and investigations.";

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

pub fn chat_completions_service_tier_override_request() -> Value {
    json!({
        "extra_body": {
            "provider": {
                "allow_fallbacks": false
            },
            "trace": "yes"
        },
        "messages": [
            {"content": "system", "role": "system"},
            {"content": "hello", "role": "user"}
        ],
        "model": "fake/model",
        "service_tier": "priority",
        "temperature": 0.2,
        "timeout": 5
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

pub fn codex_responses_id_helpers_fixture() -> Value {
    json!({
        "deterministic": {
            "terminal_zero": codex_deterministic_call_id("terminal", "{\"cmd\":\"ls\"}", 0),
            "terminal_one": codex_deterministic_call_id("terminal", "{\"cmd\":\"ls\"}", 1),
            "unicode": codex_deterministic_call_id("unicode", "{\"text\":\"olá\"}", 2),
        },
        "split": {
            "pipe": codex_split_responses_tool_id_value(&json!(" call_abc | fc_def ")),
            "fc_only": codex_split_responses_tool_id_value(&json!("fc_response_item")),
            "call_only": codex_split_responses_tool_id_value(&json!("call_plain")),
            "empty": codex_split_responses_tool_id_value(&json!("  ")),
            "nonstr": codex_split_responses_tool_id_value(&json!(42)),
        },
        "derive": {
            "response_item_wins": codex_derive_responses_function_call_id("call_abc", Some(" fc_existing ")),
            "call_prefix": codex_derive_responses_function_call_id("call_abc", None),
            "already_fc": codex_derive_responses_function_call_id("fc_raw", None),
            "sanitized": codex_derive_responses_function_call_id("weird id/!*", None),
            "response_seed": codex_derive_responses_function_call_id("", Some("notfc")),
        },
    })
}

pub fn codex_responses_input_conversion_fixture() -> Value {
    let messages = codex_fixture_messages();
    json!({
        "standard": chat_messages_to_responses_input(&messages, false),
        "xai": chat_messages_to_responses_input(&messages, true),
    })
}

pub fn codex_responses_preflight_fixture() -> Value {
    let input = preflight_items_input();
    json!({
        "items": preflight_codex_input_items(&Value::Array(input.clone())).unwrap(),
        "errors": {
            "not_list": preflight_error(&json!({"bad": true})),
            "bad_tool_output": preflight_error(&json!([
                {"type": "function_call_output", "output": "missing call"}
            ])),
            "bad_message_role": preflight_error(&json!([
                {"type": "message", "role": "user", "content": []}
            ])),
            "bad_content_part": preflight_error(&json!([
                {"role": "user", "content": [{"type": "unsupported"}]}
            ])),
        },
        "api_kwargs": preflight_codex_api_kwargs(&json!({
            "model": " gpt-5.4 ",
            "instructions": " ",
            "input": [input[0].clone(), input[1].clone()],
            "tools": [
                {
                    "type": "function",
                    "name": " terminal ",
                    "description": 123,
                    "strict": "yes",
                    "parameters": {"type": "object"},
                }
            ],
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "reasoning": {"effort": "medium"},
            "max_output_tokens": 99.9,
            "temperature": 0,
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "prompt_cache_key": "session-x",
            "service_tier": " priority ",
            "extra_headers": {" X-Test ": 7, "Skip": null},
            "extra_body": {"prompt_cache_key": "session-x"},
        })).unwrap(),
    })
}

pub fn codex_response_normalization_fixture() -> Value {
    let response = json!({
        "status": "incomplete",
        "output": [
            {
                "type": "reasoning",
                "id": "rs_resp",
                "encrypted_content": "enc-resp",
                "summary": [{"text": "reason one"}],
                "status": "completed",
            },
            {
                "type": "message",
                "id": "msg_resp",
                "status": "completed",
                "phase": "commentary",
                "content": [{"type": "output_text", "text": "thinking aloud"}],
            },
            {
                "type": "function_call",
                "id": "fc_tool",
                "call_id": "",
                "name": "terminal",
                "arguments": {"cmd": "ls"},
                "status": "completed",
            },
            {
                "type": "custom_tool_call",
                "id": "custom|fc_custom",
                "call_id": null,
                "name": "custom_tool",
                "input": ["raw"],
                "status": "completed",
            },
        ],
    });
    normalize_codex_response(&response)
}

fn codex_fixture_messages() -> Vec<Value> {
    vec![
        json!({"role": "system", "content": "ignored system"}),
        json!({
            "role": "user",
            "content": [
                "lead",
                {"type": "text", "text": "hello"},
                {
                    "type": "image_url",
                    "image_url": {"url": "https://example.invalid/a.png", "detail": "low"}
                },
                {"type": "unknown", "text": "ignored"}
            ],
        }),
        json!({
            "role": "assistant",
            "content": "",
            "codex_reasoning_items": [
                {
                    "id": "rs_1",
                    "type": "reasoning",
                    "encrypted_content": "enc-1",
                    "summary": [{"type": "summary_text", "text": "sum"}],
                },
                {
                    "id": "rs_1",
                    "type": "reasoning",
                    "encrypted_content": "enc-duplicate",
                },
            ],
            "codex_message_items": [
                {
                    "id": "msg_1",
                    "type": "message",
                    "role": "assistant",
                    "status": "in progress",
                    "phase": "final_answer",
                    "content": [
                        {"type": "text", "text": "prior"},
                        {"type": "ignored", "text": "drop"},
                    ],
                }
            ],
            "tool_calls": [
                {
                    "id": " call_embedded | fc_item ",
                    "type": "function",
                    "function": {"name": "terminal", "arguments": {"cmd": "pwd"}},
                },
                {
                    "id": "fc_only_item",
                    "type": "function",
                    "function": {"name": "read_file", "arguments": " {\"path\":\"x\"} "},
                },
                {
                    "type": "function",
                    "function": {"name": "missing_id", "arguments": {"a": 1}},
                },
            ],
        }),
        json!({
            "role": "tool",
            "tool_call_id": " call_embedded | fc_item ",
            "content": [
                {"type": "text", "text": "tool output"},
                {
                    "type": "image_url",
                    "image_url": "https://example.invalid/tool.png",
                    "detail": "high",
                },
                {"type": "unknown", "text": "drop"},
            ],
        }),
        json!({"role": "tool", "tool_call_id": "", "content": "ignored"}),
    ]
}

fn preflight_items_input() -> Vec<Value> {
    vec![
        json!({"type": "function_call", "call_id": " call_1 ", "name": " terminal ", "arguments": {"cmd": "ls"}}),
        json!({
            "type": "function_call_output",
            "call_id": " call_1 ",
            "output": [
                {"type": "input_text", "text": "ok"},
                {"type": "input_image", "image_url": "https://example.invalid/i.png", "detail": " low "},
                {"type": "input_image", "image_url": ""},
                {"type": "bad", "text": "drop"},
            ],
        }),
        json!({"type": "reasoning", "id": "rs_a", "encrypted_content": "enc-a", "summary": ["raw"]}),
        json!({"type": "reasoning", "id": "rs_a", "encrypted_content": "enc-b"}),
        json!({
            "type": "message",
            "role": "assistant",
            "status": "IN-PROGRESS",
            "id": " msg_keep ",
            "phase": " commentary ",
            "content": [{"type": "text", "text": 123}],
        }),
        json!({
            "role": "assistant",
            "content": [
                "inline",
                {"type": "input_text", "text": "assistant text"},
                {"type": "input_image", "image_url": "https://example.invalid/ignored.png"},
            ],
        }),
        json!({
            "role": "user",
            "content": [
                {"type": "output_text", "text": "coerced"},
                {"type": "image_url", "image_url": {"url": "https://example.invalid/user.png", "detail": "auto"}},
            ],
        }),
    ]
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

fn codex_deterministic_call_id(fn_name: &str, arguments: &str, index: usize) -> String {
    let seed = format!("{fn_name}:{arguments}:{index}");
    let digest = Sha256::digest(seed.as_bytes());
    format!("call_{}", hex_prefix(&digest, 12))
}

fn codex_split_responses_tool_id(raw_id: &Value) -> (Option<String>, Option<String>) {
    let Some(raw) = raw_id.as_str() else {
        return (None, None);
    };
    let value = raw.trim();
    if value.is_empty() {
        return (None, None);
    }
    if let Some((call_id, response_item_id)) = value.split_once('|') {
        return (
            nonempty_trimmed(call_id),
            nonempty_trimmed(response_item_id),
        );
    }
    if value.starts_with("fc_") {
        return (None, Some(value.to_string()));
    }
    (Some(value.to_string()), None)
}

fn codex_split_responses_tool_id_value(raw_id: &Value) -> Value {
    let (call_id, response_item_id) = codex_split_responses_tool_id(raw_id);
    json!([call_id, response_item_id])
}

fn codex_derive_responses_function_call_id(
    call_id: &str,
    response_item_id: Option<&str>,
) -> String {
    if let Some(candidate) = response_item_id.map(str::trim) {
        if candidate.starts_with("fc_") {
            return candidate.to_string();
        }
    }

    let source = call_id.trim();
    if source.starts_with("fc_") {
        return source.to_string();
    }
    if let Some(suffix) = source.strip_prefix("call_") {
        if !suffix.is_empty() {
            return format!("fc_{suffix}");
        }
    }

    let sanitized = source
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        .collect::<String>();
    if sanitized.starts_with("fc_") {
        return sanitized;
    }
    if let Some(suffix) = sanitized.strip_prefix("call_") {
        if !suffix.is_empty() {
            return format!("fc_{suffix}");
        }
    }
    if !sanitized.is_empty() {
        return format!("fc_{}", truncate_chars(&sanitized, 48));
    }

    let seed = if source.is_empty() {
        response_item_id.unwrap_or_default().to_string()
    } else {
        source.to_string()
    };
    let digest = Sha1::digest(seed.as_bytes());
    format!("fc_{}", hex_prefix(&digest, 24))
}

fn chat_messages_to_responses_input(messages: &[Value], is_xai_responses: bool) -> Vec<Value> {
    let mut items = Vec::new();
    let mut seen_item_ids = Vec::<String>::new();

    for msg in messages {
        if !msg.is_object() {
            continue;
        }
        let role = get_str(msg, "role");
        if role == "system" {
            continue;
        }

        if matches!(role, "user" | "assistant") {
            let content = msg.get("content").unwrap_or(&Value::Null);
            let (content_parts, content_text) = if content.is_array() {
                let parts = chat_content_to_responses_parts(content, role);
                let text_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
                let text = parts
                    .iter()
                    .filter(|part| get_str(part, "type") == text_type)
                    .map(|part| get_str(part, "text"))
                    .collect::<String>();
                (parts, text)
            } else {
                (Vec::new(), value_to_python_str(content))
            };

            if role == "assistant" {
                let mut has_codex_reasoning = false;
                if !is_xai_responses {
                    if let Some(reasoning_items) =
                        msg.get("codex_reasoning_items").and_then(Value::as_array)
                    {
                        for reasoning_item in reasoning_items {
                            let encrypted = get_str(reasoning_item, "encrypted_content");
                            if encrypted.is_empty() {
                                continue;
                            }
                            let item_id = get_str(reasoning_item, "id");
                            if !item_id.is_empty() && seen_item_ids.iter().any(|id| id == item_id) {
                                continue;
                            }
                            let mut replay_item = reasoning_item.as_object().unwrap().clone();
                            replay_item.remove("id");
                            items.push(Value::Object(replay_item));
                            if !item_id.is_empty() {
                                seen_item_ids.push(item_id.to_string());
                            }
                            has_codex_reasoning = true;
                        }
                    }
                }

                let mut replayed_message_items = 0;
                if let Some(message_items) =
                    msg.get("codex_message_items").and_then(Value::as_array)
                {
                    for raw_item in message_items {
                        if raw_item.get("type").and_then(Value::as_str) != Some("message")
                            || raw_item.get("role").and_then(Value::as_str) != Some("assistant")
                        {
                            continue;
                        }
                        let Some(raw_content_parts) =
                            raw_item.get("content").and_then(Value::as_array)
                        else {
                            continue;
                        };
                        let normalized_content_parts = raw_content_parts
                            .iter()
                            .filter_map(|part| {
                                let part_type = get_str(part, "type").trim();
                                if !matches!(part_type, "output_text" | "text") {
                                    return None;
                                }
                                Some(json!({
                                    "type": "output_text",
                                    "text": value_to_python_str(part.get("text").unwrap_or(&Value::String(String::new()))),
                                }))
                            })
                            .collect::<Vec<_>>();
                        if normalized_content_parts.is_empty() {
                            continue;
                        }
                        let mut replay_item = json!({
                            "type": "message",
                            "role": "assistant",
                            "status": normalize_responses_message_status(raw_item.get("status")),
                            "content": normalized_content_parts,
                        });
                        if let Some(object) = replay_item.as_object_mut() {
                            let item_id = get_str(raw_item, "id").trim();
                            if !item_id.is_empty() {
                                object.insert("id".to_string(), json!(item_id));
                            }
                            let phase = get_str(raw_item, "phase").trim();
                            if !phase.is_empty() {
                                object.insert("phase".to_string(), json!(phase));
                            }
                        }
                        items.push(replay_item);
                        replayed_message_items += 1;
                    }
                }

                if replayed_message_items == 0 {
                    if !content_parts.is_empty() {
                        items.push(json!({"role": "assistant", "content": content_parts}));
                    } else if !content_text.trim().is_empty() {
                        items.push(json!({"role": "assistant", "content": content_text}));
                    } else if has_codex_reasoning {
                        items.push(json!({"role": "assistant", "content": ""}));
                    }
                }

                if let Some(tool_calls) = msg.get("tool_calls").and_then(Value::as_array) {
                    for tool_call in tool_calls {
                        let function = tool_call.get("function").unwrap_or(&Value::Null);
                        let fn_name = get_str(function, "name");
                        if fn_name.trim().is_empty() {
                            continue;
                        }
                        let (embedded_call_id, embedded_response_item_id) =
                            codex_split_responses_tool_id(
                                tool_call.get("id").unwrap_or(&Value::Null),
                            );
                        let mut call_id = tool_call
                            .get("call_id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                            .or(embedded_call_id);
                        if call_id.as_ref().is_none_or(|value| value.trim().is_empty()) {
                            if let Some(response_item_id) = embedded_response_item_id {
                                if let Some(suffix) = response_item_id.strip_prefix("fc_") {
                                    if !suffix.is_empty() {
                                        call_id = Some(format!("call_{suffix}"));
                                    }
                                }
                            }
                        }
                        let arguments_value = function
                            .get("arguments")
                            .cloned()
                            .unwrap_or_else(|| json!("{}"));
                        let raw_arguments_for_id = python_repr_for_str(&arguments_value);
                        let mut arguments =
                            if arguments_value.is_object() || arguments_value.is_array() {
                                python_json_in_order(&arguments_value)
                            } else if let Some(value) = arguments_value.as_str() {
                                value.to_string()
                            } else {
                                value_to_python_str(&arguments_value)
                            };
                        arguments = arguments.trim().to_string();
                        if arguments.is_empty() {
                            arguments = "{}".to_string();
                        }
                        let call_id = call_id.unwrap_or_else(|| {
                            codex_deterministic_call_id(fn_name, &raw_arguments_for_id, items.len())
                        });
                        items.push(json!({
                            "type": "function_call",
                            "call_id": call_id.trim(),
                            "name": fn_name,
                            "arguments": arguments,
                        }));
                    }
                }
                continue;
            }

            if !content_parts.is_empty() {
                items.push(json!({"role": role, "content": content_parts}));
            } else {
                items.push(json!({"role": role, "content": content_text}));
            }
            continue;
        }

        if role == "tool" {
            let raw_tool_call_id = msg.get("tool_call_id").unwrap_or(&Value::Null);
            let (mut call_id, _) = codex_split_responses_tool_id(raw_tool_call_id);
            if call_id.as_ref().is_none_or(|value| value.trim().is_empty()) {
                call_id = raw_tool_call_id
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned);
            }
            let Some(call_id) = call_id else {
                continue;
            };
            let tool_content = msg.get("content").unwrap_or(&Value::Null);
            let output = if tool_content.is_array() {
                let converted = chat_content_to_responses_parts(tool_content, "user");
                if converted.is_empty() {
                    json!("")
                } else {
                    Value::Array(converted)
                }
            } else {
                json!(value_to_python_str(tool_content))
            };
            items.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": output,
            }));
        }
    }

    items
}

fn preflight_error(raw_items: &Value) -> Value {
    match preflight_codex_input_items(raw_items) {
        Ok(_) => json!({"type": null, "message": null}),
        Err(message) => json!({"type": "ValueError", "message": message}),
    }
}

fn preflight_codex_input_items(raw_items: &Value) -> Result<Value, String> {
    let Some(items) = raw_items.as_array() else {
        return Err("Codex Responses input must be a list of input items.".to_string());
    };
    let mut normalized = Vec::new();
    let mut seen_ids = Vec::<String>::new();
    for (idx, item) in items.iter().enumerate() {
        if !item.is_object() {
            return Err(format!("Codex Responses input[{idx}] must be an object."));
        }
        let item_type = get_str(item, "type");
        if item_type == "function_call" {
            let call_id = get_str(item, "call_id").trim();
            let name = get_str(item, "name").trim();
            if call_id.is_empty() {
                return Err(format!(
                    "Codex Responses input[{idx}] function_call is missing call_id."
                ));
            }
            if name.is_empty() {
                return Err(format!(
                    "Codex Responses input[{idx}] function_call is missing name."
                ));
            }
            let arguments_value = item
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!("{}"));
            let mut arguments = if arguments_value.is_object() || arguments_value.is_array() {
                python_json_in_order(&arguments_value)
            } else if let Some(value) = arguments_value.as_str() {
                value.to_string()
            } else {
                value_to_python_str(&arguments_value)
            };
            arguments = arguments.trim().to_string();
            if arguments.is_empty() {
                arguments = "{}".to_string();
            }
            normalized.push(json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": arguments,
            }));
            continue;
        }
        if item_type == "function_call_output" {
            let call_id = get_str(item, "call_id").trim();
            if call_id.is_empty() {
                return Err(format!(
                    "Codex Responses input[{idx}] function_call_output is missing call_id."
                ));
            }
            let output = item.get("output").unwrap_or(&Value::Null);
            if let Some(parts) = output.as_array() {
                let mut cleaned = Vec::new();
                for part in parts {
                    match get_str(part, "type") {
                        "input_text" => {
                            let text = get_str(part, "text");
                            if !text.is_empty() {
                                cleaned.push(json!({"type": "input_text", "text": text}));
                            }
                        }
                        "input_image" => {
                            let url = get_str(part, "image_url");
                            if !url.is_empty() {
                                let mut entry = json!({"type": "input_image", "image_url": url});
                                let detail = get_str(part, "detail").trim();
                                if !detail.is_empty() {
                                    entry
                                        .as_object_mut()
                                        .unwrap()
                                        .insert("detail".to_string(), json!(detail));
                                }
                                cleaned.push(entry);
                            }
                        }
                        _ => {}
                    }
                }
                normalized.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": if cleaned.is_empty() { json!("") } else { Value::Array(cleaned) },
                }));
                continue;
            }
            normalized.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": value_to_python_str(output),
            }));
            continue;
        }
        if item_type == "reasoning" {
            let encrypted = get_str(item, "encrypted_content");
            if !encrypted.is_empty() {
                let item_id = get_str(item, "id");
                if !item_id.is_empty() {
                    if seen_ids.iter().any(|id| id == item_id) {
                        continue;
                    }
                    seen_ids.push(item_id.to_string());
                }
                let summary = item
                    .get("summary")
                    .filter(|value| value.is_array())
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                normalized.push(json!({
                    "type": "reasoning",
                    "encrypted_content": encrypted,
                    "summary": summary,
                }));
            }
            continue;
        }
        if item_type == "message" {
            if get_str(item, "role") != "assistant" {
                return Err(format!(
                    "Codex Responses input[{idx}] message items must have role='assistant'."
                ));
            }
            let Some(content) = item.get("content").and_then(Value::as_array) else {
                return Err(format!(
                    "Codex Responses input[{idx}] message item must have content list."
                ));
            };
            let mut normalized_content = Vec::new();
            for (part_idx, part) in content.iter().enumerate() {
                if !part.is_object() {
                    return Err(format!(
                        "Codex Responses input[{idx}] message content[{part_idx}] must be an object."
                    ));
                }
                let part_type = get_str(part, "type");
                if !matches!(part_type, "output_text" | "text") {
                    return Err(format!(
                        "Codex Responses input[{idx}] message content[{part_idx}] has unsupported type {}.",
                        python_repr(part.get("type").unwrap_or(&Value::Null))
                    ));
                }
                normalized_content.push(json!({
                    "type": "output_text",
                    "text": value_to_python_str(part.get("text").unwrap_or(&Value::String(String::new()))),
                }));
            }
            if normalized_content.is_empty() {
                return Err(format!(
                    "Codex Responses input[{idx}] message item must contain at least one text part."
                ));
            }
            let mut normalized_item = json!({
                "type": "message",
                "role": "assistant",
                "status": normalize_responses_message_status(item.get("status")),
                "content": normalized_content,
            });
            let object = normalized_item.as_object_mut().unwrap();
            let item_id = get_str(item, "id").trim();
            if !item_id.is_empty() {
                object.insert("id".to_string(), json!(item_id));
            }
            let phase = get_str(item, "phase").trim();
            if !phase.is_empty() {
                object.insert("phase".to_string(), json!(phase));
            }
            normalized.push(normalized_item);
            continue;
        }
        let role = get_str(item, "role");
        if matches!(role, "user" | "assistant") {
            let content = item.get("content").unwrap_or(&Value::Null);
            if let Some(parts) = content.as_array() {
                let text_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
                let mut validated = Vec::new();
                for (part_idx, part) in parts.iter().enumerate() {
                    if let Some(text) = part.as_str() {
                        if !text.is_empty() {
                            validated.push(json!({"type": text_type, "text": text}));
                        }
                        continue;
                    }
                    if !part.is_object() {
                        return Err(format!(
                            "Codex Responses input[{idx}].content[{part_idx}] must be an object or string."
                        ));
                    }
                    let ptype = get_str(part, "type").trim().to_ascii_lowercase();
                    if matches!(ptype.as_str(), "input_text" | "text" | "output_text") {
                        validated.push(json!({
                            "type": text_type,
                            "text": value_to_python_str(part.get("text").unwrap_or(&Value::String(String::new()))),
                        }));
                    } else if matches!(ptype.as_str(), "input_image" | "image_url") {
                        let image_ref = part.get("image_url").unwrap_or(&Value::Null);
                        let (url, detail) = if let Some(object) = image_ref.as_object() {
                            (
                                object
                                    .get("url")
                                    .map(value_to_python_str)
                                    .unwrap_or_default(),
                                object
                                    .get("detail")
                                    .or_else(|| part.get("detail"))
                                    .and_then(Value::as_str)
                                    .map(str::trim)
                                    .filter(|value| !value.is_empty())
                                    .map(ToOwned::to_owned),
                            )
                        } else {
                            (
                                value_to_python_str(image_ref),
                                part.get("detail")
                                    .and_then(Value::as_str)
                                    .map(str::trim)
                                    .filter(|value| !value.is_empty())
                                    .map(ToOwned::to_owned),
                            )
                        };
                        let mut image_part = json!({"type": "input_image", "image_url": url});
                        if let Some(detail) = detail {
                            image_part
                                .as_object_mut()
                                .unwrap()
                                .insert("detail".to_string(), json!(detail));
                        }
                        validated.push(image_part);
                    } else {
                        return Err(format!(
                            "Codex Responses input[{idx}].content[{part_idx}] has unsupported type {}.",
                            python_repr(part.get("type").unwrap_or(&Value::Null))
                        ));
                    }
                }
                normalized.push(json!({"role": role, "content": validated}));
                continue;
            }
            normalized.push(json!({"role": role, "content": value_to_python_str(content)}));
            continue;
        }
        return Err(format!(
            "Codex Responses input[{idx}] has unsupported item shape (type={item_type:?}, role={role:?})."
        ));
    }
    Ok(Value::Array(normalized))
}

fn preflight_codex_api_kwargs(api_kwargs: &Value) -> Result<Value, String> {
    let object = api_kwargs
        .as_object()
        .ok_or_else(|| "Codex Responses request must be a dict.".to_string())?;
    for required in ["model", "instructions", "input"] {
        if !object.contains_key(required) {
            return Err(format!(
                "Codex Responses request missing required field(s): {required}."
            ));
        }
    }
    let model = get_str(api_kwargs, "model").trim();
    if model.is_empty() {
        return Err("Codex Responses request 'model' must be a non-empty string.".to_string());
    }
    let mut instructions = value_to_python_str(api_kwargs.get("instructions").unwrap())
        .trim()
        .to_string();
    if instructions.is_empty() {
        instructions = DEFAULT_AGENT_IDENTITY.to_string();
    }
    let normalized_input = preflight_codex_input_items(api_kwargs.get("input").unwrap())?;
    let mut normalized = json!({
        "model": model,
        "instructions": instructions,
        "input": normalized_input,
        "store": false,
    });

    if let Some(tools) = api_kwargs.get("tools") {
        if !tools.is_null() {
            let Some(tools_array) = tools.as_array() else {
                return Err(
                    "Codex Responses request 'tools' must be a list when provided.".to_string(),
                );
            };
            let mut normalized_tools = Vec::new();
            for (idx, tool) in tools_array.iter().enumerate() {
                if !tool.is_object() {
                    return Err(format!("Codex Responses tools[{idx}] must be an object."));
                }
                if get_str(tool, "type") != "function" {
                    return Err(format!(
                        "Codex Responses tools[{idx}] has unsupported type {}.",
                        python_repr(tool.get("type").unwrap_or(&Value::Null))
                    ));
                }
                let name = get_str(tool, "name").trim();
                if name.is_empty() {
                    return Err(format!(
                        "Codex Responses tools[{idx}] is missing a valid name."
                    ));
                }
                let parameters = tool
                    .get("parameters")
                    .filter(|value| value.is_object())
                    .ok_or_else(|| {
                        format!("Codex Responses tools[{idx}] is missing valid parameters.")
                    })?;
                normalized_tools.push(json!({
                    "type": "function",
                    "name": name,
                    "description": value_to_python_str(tool.get("description").unwrap_or(&Value::String(String::new()))),
                    "strict": tool.get("strict").and_then(Value::as_bool).unwrap_or_else(|| !tool.get("strict").unwrap_or(&Value::Bool(false)).is_null()),
                    "parameters": parameters,
                }));
            }
            normalized
                .as_object_mut()
                .unwrap()
                .insert("tools".to_string(), Value::Array(normalized_tools));
        }
    }

    if api_kwargs.get("store") != Some(&Value::Bool(false)) {
        return Err("Codex Responses contract requires 'store' to be false.".to_string());
    }
    let target = normalized.as_object_mut().unwrap();
    for key in ["reasoning", "include"] {
        if let Some(value) = api_kwargs.get(key) {
            if (key == "reasoning" && value.is_object()) || (key == "include" && value.is_array()) {
                target.insert(key.to_string(), value.clone());
            }
        }
    }
    let service_tier = get_str(api_kwargs, "service_tier").trim();
    if !service_tier.is_empty() {
        target.insert("service_tier".to_string(), json!(service_tier));
    }
    if let Some(tokens) = api_kwargs.get("max_output_tokens").and_then(Value::as_f64) {
        if tokens > 0.0 {
            target.insert("max_output_tokens".to_string(), json!(tokens as i64));
        }
    }
    if let Some(temperature) = api_kwargs.get("temperature").and_then(Value::as_f64) {
        target.insert("temperature".to_string(), json!(temperature));
    }
    for key in ["tool_choice", "parallel_tool_calls", "prompt_cache_key"] {
        if let Some(value) = api_kwargs.get(key).filter(|value| !value.is_null()) {
            target.insert(key.to_string(), value.clone());
        }
    }
    if let Some(headers) = api_kwargs.get("extra_headers") {
        let mut normalized_headers = serde_json::Map::new();
        if let Some(headers) = headers.as_object() {
            for (key, value) in headers {
                let trimmed = key.trim();
                if !trimmed.is_empty() && !value.is_null() {
                    normalized_headers
                        .insert(trimmed.to_string(), json!(value_to_python_str(value)));
                }
            }
        }
        if !normalized_headers.is_empty() {
            target.insert(
                "extra_headers".to_string(),
                Value::Object(normalized_headers),
            );
        }
    }
    if let Some(extra_body) = api_kwargs
        .get("extra_body")
        .filter(|value| value.is_object())
    {
        if !extra_body.as_object().unwrap().is_empty() {
            target.insert("extra_body".to_string(), extra_body.clone());
        }
    }
    Ok(normalized)
}

fn normalize_codex_response(response: &Value) -> Value {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let response_status = response
        .get("status")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase());
    let mut content_parts = Vec::new();
    let mut reasoning_parts = Vec::new();
    let mut reasoning_items_raw = Vec::new();
    let mut message_items_raw = Vec::new();
    let mut tool_calls = Vec::new();
    let mut has_incomplete_items = matches!(
        response_status.as_deref(),
        Some("queued" | "in_progress" | "incomplete")
    );
    let mut saw_commentary_phase = false;
    let mut saw_final_answer_phase = false;

    for item in &output {
        let item_type = get_str(item, "type");
        let item_status = item
            .get("status")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase());
        if matches!(
            item_status.as_deref(),
            Some("queued" | "in_progress" | "incomplete")
        ) {
            has_incomplete_items = true;
        }
        match item_type {
            "message" => {
                let normalized_phase = item
                    .get("phase")
                    .and_then(Value::as_str)
                    .map(|value| value.trim().to_ascii_lowercase())
                    .filter(|value| !value.is_empty());
                if matches!(normalized_phase.as_deref(), Some("commentary" | "analysis")) {
                    saw_commentary_phase = true;
                } else if matches!(normalized_phase.as_deref(), Some("final_answer" | "final")) {
                    saw_final_answer_phase = true;
                }
                let message_text = extract_responses_message_text(item);
                if !message_text.is_empty() {
                    content_parts.push(message_text.clone());
                    let mut raw_message_item = json!({
                        "type": "message",
                        "role": "assistant",
                        "status": normalize_responses_message_status_str(item_status.as_deref()),
                        "content": [{"type": "output_text", "text": message_text}],
                    });
                    let object = raw_message_item.as_object_mut().unwrap();
                    let item_id = get_str(item, "id");
                    if !item_id.is_empty() {
                        object.insert("id".to_string(), json!(item_id));
                    }
                    if let Some(phase) = normalized_phase {
                        object.insert("phase".to_string(), json!(phase));
                    }
                    message_items_raw.push(raw_message_item);
                }
            }
            "reasoning" => {
                let reasoning_text = extract_responses_reasoning_text(item);
                if !reasoning_text.is_empty() {
                    reasoning_parts.push(reasoning_text);
                }
                let encrypted = get_str(item, "encrypted_content");
                if !encrypted.is_empty() {
                    let mut raw_item = json!({"type": "reasoning", "encrypted_content": encrypted});
                    let object = raw_item.as_object_mut().unwrap();
                    let item_id = get_str(item, "id");
                    if !item_id.is_empty() {
                        object.insert("id".to_string(), json!(item_id));
                    }
                    if let Some(summary) = item.get("summary").and_then(Value::as_array) {
                        let raw_summary = summary
                            .iter()
                            .filter_map(|part| {
                                part.get("text")
                                    .and_then(Value::as_str)
                                    .map(|text| json!({"type": "summary_text", "text": text}))
                            })
                            .collect::<Vec<_>>();
                        object.insert("summary".to_string(), Value::Array(raw_summary));
                    }
                    reasoning_items_raw.push(raw_item);
                }
            }
            "function_call" | "custom_tool_call" => {
                if matches!(
                    item_status.as_deref(),
                    Some("queued" | "in_progress" | "incomplete")
                ) {
                    continue;
                }
                let fn_name = get_str(item, "name");
                let argument_key = if item_type == "custom_tool_call" {
                    "input"
                } else {
                    "arguments"
                };
                let arguments_value = item
                    .get(argument_key)
                    .cloned()
                    .unwrap_or_else(|| json!("{}"));
                let arguments = if arguments_value.is_string() {
                    arguments_value.as_str().unwrap().to_string()
                } else {
                    python_json_in_order(&arguments_value)
                };
                let raw_call_id = item.get("call_id").and_then(Value::as_str).map(str::trim);
                let raw_item_id = item.get("id").unwrap_or(&Value::Null);
                let (embedded_call_id, _) = codex_split_responses_tool_id(raw_item_id);
                let call_id = raw_call_id
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .or(embedded_call_id)
                    .unwrap_or_else(|| {
                        codex_deterministic_call_id(fn_name, &arguments, tool_calls.len())
                    });
                let response_item_id = raw_item_id.as_str();
                let response_item_id =
                    codex_derive_responses_function_call_id(&call_id, response_item_id);
                tool_calls.push(json!({
                    "id": call_id,
                    "call_id": call_id,
                    "response_item_id": response_item_id,
                    "type": "function",
                    "function": {
                        "name": fn_name,
                        "arguments": arguments,
                    },
                }));
            }
            _ => {}
        }
    }

    let final_text = content_parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls"
    } else if has_incomplete_items
        || (saw_commentary_phase && !saw_final_answer_phase)
        || (!reasoning_items_raw.is_empty() && final_text.is_empty())
    {
        "incomplete"
    } else {
        "stop"
    };
    json!({
        "finish_reason": finish_reason,
        "message": {
            "content": final_text,
            "tool_calls": tool_calls,
            "reasoning": if reasoning_parts.is_empty() { Value::Null } else { json!(reasoning_parts.join("\n\n")) },
            "codex_reasoning_items": if reasoning_items_raw.is_empty() { Value::Null } else { Value::Array(reasoning_items_raw) },
            "codex_message_items": if message_items_raw.is_empty() { Value::Null } else { Value::Array(message_items_raw) },
        },
    })
}

fn chat_content_to_responses_parts(content: &Value, role: &str) -> Vec<Value> {
    let text_type = if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };
    let Some(parts) = content.as_array() else {
        return Vec::new();
    };
    let mut converted = Vec::new();
    for part in parts {
        if let Some(text) = part.as_str() {
            if !text.is_empty() {
                converted.push(json!({"type": text_type, "text": text}));
            }
            continue;
        }
        if !part.is_object() {
            continue;
        }
        let ptype = get_str(part, "type").trim().to_ascii_lowercase();
        if matches!(ptype.as_str(), "text" | "input_text" | "output_text") {
            let text = part.get("text").and_then(Value::as_str).unwrap_or("");
            if !text.is_empty() {
                converted.push(json!({"type": text_type, "text": text}));
            }
        } else if matches!(ptype.as_str(), "image_url" | "input_image") {
            let image_ref = part.get("image_url").unwrap_or(&Value::Null);
            let (url, detail) = if let Some(object) = image_ref.as_object() {
                (
                    object.get("url").and_then(Value::as_str).unwrap_or(""),
                    object
                        .get("detail")
                        .or_else(|| part.get("detail"))
                        .and_then(Value::as_str),
                )
            } else {
                (
                    image_ref.as_str().unwrap_or(""),
                    part.get("detail").and_then(Value::as_str),
                )
            };
            if !url.is_empty() {
                let mut image_part = json!({"type": "input_image", "image_url": url});
                if let Some(detail) = detail.map(str::trim).filter(|value| !value.is_empty()) {
                    image_part
                        .as_object_mut()
                        .unwrap()
                        .insert("detail".to_string(), json!(detail));
                }
                converted.push(image_part);
            }
        }
    }
    converted
}

fn normalize_responses_message_status(value: Option<&Value>) -> &'static str {
    if let Some(raw) = value.and_then(Value::as_str) {
        return normalize_responses_message_status_str(Some(raw));
    }
    "completed"
}

fn normalize_responses_message_status_str(value: Option<&str>) -> &'static str {
    if let Some(raw) = value {
        let status = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if matches!(status.as_str(), "completed" | "incomplete" | "in_progress") {
            return match status.as_str() {
                "incomplete" => "incomplete",
                "in_progress" => "in_progress",
                _ => "completed",
            };
        }
    }
    "completed"
}

fn extract_responses_message_text(item: &Value) -> String {
    item.get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| matches!(get_str(part, "type"), "output_text" | "text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .filter(|text| !text.is_empty())
        .collect::<String>()
        .trim()
        .to_string()
}

fn extract_responses_reasoning_text(item: &Value) -> String {
    if let Some(summary) = item.get("summary").and_then(Value::as_array) {
        let chunks = summary
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        if !chunks.is_empty() {
            return chunks.join("\n").trim().to_string();
        }
    }
    get_str(item, "text").trim().to_string()
}

fn value_to_python_str(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn python_repr(value: &Value) -> String {
    match value {
        Value::String(value) => format!("'{value}'"),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        _ => value.to_string(),
    }
}

fn python_repr_for_str(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let parts = object
                .iter()
                .map(|(key, value)| format!("'{key}': {}", python_repr_for_str(value)))
                .collect::<Vec<_>>();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Array(items) => {
            let parts = items.iter().map(python_repr_for_str).collect::<Vec<_>>();
            format!("[{}]", parts.join(", "))
        }
        Value::String(value) => value.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        _ => value.to_string(),
    }
}

fn nonempty_trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn hex_prefix(bytes: &[u8], chars: usize) -> String {
    let mut out = String::with_capacity(chars);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
        if out.len() >= chars {
            out.truncate(chars);
            break;
        }
    }
    out
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

pub fn stream_diag_capture_response(status_code: i64, headers: &Value) -> Value {
    let mut captured = serde_json::Map::new();
    for name in STREAM_DIAG_HEADERS {
        if let Some(value) = headers.get(name).and_then(Value::as_str) {
            if !value.is_empty() {
                captured.insert(name.to_string(), json!(truncate_chars(value, 120)));
            }
        }
    }
    json!({
        "started_at": 1000.0,
        "first_chunk_at": null,
        "chunks": 0,
        "bytes": 0,
        "headers": captured,
        "http_status": status_code,
    })
}

pub fn flatten_exception_chain(parts: &[(&str, &str)]) -> String {
    parts
        .iter()
        .take(4)
        .map(|(class_name, message)| {
            let mut message = message.trim().replace('\n', " ");
            if message.chars().count() > 140 {
                message = format!("{}\u{2026}", truncate_chars(&message, 140));
            }
            if message.is_empty() {
                (*class_name).to_string()
            } else {
                format!("{class_name}({message})")
            }
        })
        .collect::<Vec<_>>()
        .join(" <- ")
}

pub fn stream_drop_emit_events(
    provider: &str,
    error_type: &str,
    attempt: i64,
    max_attempts: i64,
    mid_tool_call: bool,
    started_at: f64,
    now: f64,
) -> Value {
    let kind = if mid_tool_call {
        "drop mid tool-call"
    } else {
        "drop"
    };
    let suffix = format!(" after {:.1}s", (now - started_at).max(0.0));
    json!({
        "status_events": [
            format!(
                "\u{26a0}\u{fe0f} {provider} stream {kind} ({error_type}){suffix} \u{2014} reconnecting, retry {attempt}/{max_attempts}"
            )
        ],
        "activity_events": [
            format!("stream retry {attempt}/{max_attempts} after {error_type}")
        ],
    })
}

pub fn classify_provider_error(input: &Value) -> Value {
    let error_type = get_str(input, "error_type");
    let raw_message = get_str(input, "message");
    let body = input.get("body").unwrap_or(&Value::Null);
    let mut status_code = input.get("status_code").and_then(Value::as_i64);
    if status_code.is_none() && error_type == "RateLimitError" {
        status_code = Some(429);
    }
    let body_message = extract_body_message(body);
    let body_code = extract_body_code(body);
    let mut haystack = raw_message.to_ascii_lowercase();
    if let Some(message) = body_message.as_ref() {
        let lower = message.to_ascii_lowercase();
        if !lower.is_empty() && !haystack.contains(&lower) {
            haystack.push(' ');
            haystack.push_str(&lower);
        }
    }
    let result_message = body_message
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| truncate_chars(raw_message, 500));
    let approx_tokens = input
        .get("approx_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let context_length = input
        .get("context_length")
        .and_then(Value::as_i64)
        .unwrap_or(200000);
    let num_messages = input
        .get("num_messages")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    if status_code == Some(400) && haystack.contains("signature") && haystack.contains("thinking") {
        return classified(
            "thinking_signature",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    if status_code == Some(429)
        && haystack.contains("extra usage")
        && haystack.contains("long context")
    {
        return classified(
            "long_context_tier",
            status_code,
            &result_message,
            true,
            true,
            false,
            false,
        );
    }
    if status_code == Some(400)
        && (haystack.contains("error parsing grammar")
            || haystack.contains("json-schema-to-grammar")
            || (haystack.contains("unable to generate parser") && haystack.contains("template")))
    {
        return classified(
            "llama_cpp_grammar_pattern",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    if haystack.contains("do not have an active grok subscription")
        || (haystack.contains("out of available resources") && haystack.contains("grok"))
    {
        return classified(
            "auth",
            status_code,
            &result_message,
            false,
            false,
            false,
            true,
        );
    }

    if let Some(status) = status_code {
        let status_result = match status {
            401 => Some(classified(
                "auth",
                status_code,
                &result_message,
                false,
                false,
                true,
                true,
            )),
            403 if haystack.contains("key limit exceeded")
                || haystack.contains("spending limit") =>
            {
                Some(classified(
                    "billing",
                    status_code,
                    &result_message,
                    false,
                    false,
                    true,
                    true,
                ))
            }
            403 => Some(classified(
                "auth",
                status_code,
                &result_message,
                false,
                false,
                false,
                true,
            )),
            402 if has_any(&haystack, USAGE_LIMIT_PATTERNS)
                && has_any(&haystack, USAGE_LIMIT_TRANSIENT_SIGNALS) =>
            {
                Some(classified(
                    "rate_limit",
                    status_code,
                    &result_message,
                    true,
                    false,
                    true,
                    true,
                ))
            }
            402 => Some(classified(
                "billing",
                status_code,
                &result_message,
                false,
                false,
                true,
                true,
            )),
            404 if has_any(&haystack, PROVIDER_POLICY_BLOCKED_PATTERNS) => Some(classified(
                "provider_policy_blocked",
                status_code,
                &result_message,
                false,
                false,
                false,
                false,
            )),
            404 if has_any(&haystack, MODEL_NOT_FOUND_PATTERNS) => Some(classified(
                "model_not_found",
                status_code,
                &result_message,
                false,
                false,
                false,
                true,
            )),
            404 => Some(classified(
                "unknown",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            413 => Some(classified(
                "payload_too_large",
                status_code,
                &result_message,
                true,
                true,
                false,
                false,
            )),
            429 => Some(classified(
                "rate_limit",
                status_code,
                &result_message,
                true,
                false,
                true,
                true,
            )),
            400 => Some(classify_bad_request(
                &haystack,
                status_code,
                &result_message,
                body,
                approx_tokens,
                context_length,
                num_messages,
            )),
            500 | 502 => Some(classified(
                "server_error",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            503 | 529 => Some(classified(
                "overloaded",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            code if (400..500).contains(&code) => Some(classified(
                "format_error",
                status_code,
                &result_message,
                false,
                false,
                false,
                true,
            )),
            500..=599 => Some(classified(
                "server_error",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            _ => None,
        };
        if let Some(result) = status_result {
            return result;
        }
    }

    if let Some(result) = classify_by_error_code(&body_code, &result_message, status_code) {
        return result;
    }
    if let Some(result) = classify_by_message(&haystack, status_code, &result_message) {
        return result;
    }
    if has_any(&haystack, SSL_TRANSIENT_PATTERNS) {
        return classified(
            "timeout",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    if status_code.is_none() && has_any(&haystack, SERVER_DISCONNECT_PATTERNS) {
        let is_large = approx_tokens as f64 > context_length as f64 * 0.6
            || (context_length <= 256000 && (approx_tokens > 120000 || num_messages > 200));
        return if is_large {
            classified(
                "context_overflow",
                status_code,
                &result_message,
                true,
                true,
                false,
                false,
            )
        } else {
            classified(
                "timeout",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )
        };
    }
    if TRANSPORT_ERROR_TYPES.contains(&error_type) {
        return classified(
            "timeout",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    classified(
        "unknown",
        status_code,
        &result_message,
        true,
        false,
        false,
        false,
    )
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

fn classified(
    reason: &str,
    status_code: Option<i64>,
    message: &str,
    retryable: bool,
    should_compress: bool,
    should_rotate_credential: bool,
    should_fallback: bool,
) -> Value {
    json!({
        "reason": reason,
        "status_code": status_code,
        "message": message,
        "retryable": retryable,
        "should_compress": should_compress,
        "should_rotate_credential": should_rotate_credential,
        "should_fallback": should_fallback,
        "is_auth": matches!(reason, "auth" | "auth_permanent"),
    })
}

fn classify_bad_request(
    haystack: &str,
    status_code: Option<i64>,
    message: &str,
    body: &Value,
    approx_tokens: i64,
    context_length: i64,
    num_messages: i64,
) -> Value {
    let body_code = extract_body_code(body);
    if has_any(haystack, IMAGE_TOO_LARGE_PATTERNS) {
        return classified(
            "image_too_large",
            status_code,
            message,
            true,
            false,
            false,
            false,
        );
    }
    if has_any(haystack, CONTEXT_OVERFLOW_PATTERNS) {
        return classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        );
    }
    if has_any(haystack, PROVIDER_POLICY_BLOCKED_PATTERNS) {
        return classified(
            "provider_policy_blocked",
            status_code,
            message,
            false,
            false,
            false,
            false,
        );
    }
    if has_any(haystack, MODEL_NOT_FOUND_PATTERNS) {
        return classified(
            "model_not_found",
            status_code,
            message,
            false,
            false,
            false,
            true,
        );
    }
    if has_any(haystack, RATE_LIMIT_PATTERNS) || matches!(body_code.as_str(), "resource_exhausted")
    {
        return classified("rate_limit", status_code, message, true, false, true, true);
    }
    if has_any(haystack, BILLING_PATTERNS) {
        return classified("billing", status_code, message, false, false, true, true);
    }

    let body_msg = extract_body_message(body)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let is_generic = body_msg.len() < 30 || matches!(body_msg.as_str(), "error" | "");
    let is_large = approx_tokens as f64 > context_length as f64 * 0.4
        || (context_length <= 256000 && (approx_tokens > 80000 || num_messages > 80));
    if is_generic && is_large {
        return classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        );
    }
    classified(
        "format_error",
        status_code,
        message,
        false,
        false,
        false,
        true,
    )
}

fn classify_by_error_code(code: &str, message: &str, status_code: Option<i64>) -> Option<Value> {
    let lower = code.to_ascii_lowercase();
    match lower.as_str() {
        "resource_exhausted" | "throttled" | "rate_limit_exceeded" => Some(classified(
            "rate_limit",
            status_code,
            message,
            true,
            false,
            true,
            false,
        )),
        "insufficient_quota" | "billing_not_active" | "payment_required" => Some(classified(
            "billing",
            status_code,
            message,
            false,
            false,
            true,
            true,
        )),
        "model_not_found" | "model_not_available" | "invalid_model" => Some(classified(
            "model_not_found",
            status_code,
            message,
            false,
            false,
            false,
            true,
        )),
        "context_length_exceeded" | "max_tokens_exceeded" => Some(classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        )),
        _ => None,
    }
}

fn classify_by_message(haystack: &str, status_code: Option<i64>, message: &str) -> Option<Value> {
    if has_any(haystack, PAYLOAD_TOO_LARGE_PATTERNS) {
        return Some(classified(
            "payload_too_large",
            status_code,
            message,
            true,
            true,
            false,
            false,
        ));
    }
    if has_any(haystack, IMAGE_TOO_LARGE_PATTERNS) {
        return Some(classified(
            "image_too_large",
            status_code,
            message,
            true,
            false,
            false,
            false,
        ));
    }
    if has_any(haystack, USAGE_LIMIT_PATTERNS) {
        return Some(if has_any(haystack, USAGE_LIMIT_TRANSIENT_SIGNALS) {
            classified("rate_limit", status_code, message, true, false, true, true)
        } else {
            classified("billing", status_code, message, false, false, true, true)
        });
    }
    if has_any(haystack, BILLING_PATTERNS) {
        return Some(classified(
            "billing",
            status_code,
            message,
            false,
            false,
            true,
            true,
        ));
    }
    if has_any(haystack, RATE_LIMIT_PATTERNS) {
        return Some(classified(
            "rate_limit",
            status_code,
            message,
            true,
            false,
            true,
            true,
        ));
    }
    if has_any(haystack, CONTEXT_OVERFLOW_PATTERNS) {
        return Some(classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        ));
    }
    if has_any(haystack, AUTH_PATTERNS) {
        return Some(classified(
            "auth",
            status_code,
            message,
            false,
            false,
            true,
            true,
        ));
    }
    if has_any(haystack, PROVIDER_POLICY_BLOCKED_PATTERNS) {
        return Some(classified(
            "provider_policy_blocked",
            status_code,
            message,
            false,
            false,
            false,
            false,
        ));
    }
    if has_any(haystack, MODEL_NOT_FOUND_PATTERNS) {
        return Some(classified(
            "model_not_found",
            status_code,
            message,
            false,
            false,
            false,
            true,
        ));
    }
    if has_any(haystack, TIMEOUT_MESSAGE_PATTERNS) {
        return Some(classified(
            "timeout",
            status_code,
            message,
            true,
            false,
            false,
            false,
        ));
    }
    None
}

fn extract_body_message(body: &Value) -> Option<String> {
    body.get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .map(|message| truncate_chars(message.trim(), 500))
}

fn extract_body_code(body: &Value) -> String {
    body.get("error")
        .and_then(|error| error.get("code").or_else(|| error.get("type")))
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .or_else(|| {
            body.get("code")
                .or_else(|| body.get("error_code"))
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn has_any(haystack: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| haystack.contains(pattern))
}

const STREAM_DIAG_HEADERS: &[&str] = &[
    "cf-ray",
    "cf-cache-status",
    "x-openrouter-provider",
    "x-openrouter-model",
    "x-openrouter-id",
    "x-request-id",
    "x-vercel-id",
    "via",
    "server",
    "x-forwarded-for",
];

const BILLING_PATTERNS: &[&str] = &[
    "insufficient credits",
    "insufficient_quota",
    "insufficient balance",
    "credit balance",
    "credits have been exhausted",
    "top up your credits",
    "payment required",
    "billing hard limit",
    "exceeded your current quota",
    "account is deactivated",
    "plan does not include",
];

const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "too many requests",
    "throttled",
    "requests per minute",
    "tokens per minute",
    "requests per day",
    "try again in",
    "please retry after",
    "resource_exhausted",
    "rate increased too quickly",
    "throttlingexception",
    "too many concurrent requests",
    "servicequotaexceededexception",
];

const USAGE_LIMIT_PATTERNS: &[&str] = &[
    "usage limit",
    "quota",
    "limit exceeded",
    "key limit exceeded",
];

const USAGE_LIMIT_TRANSIENT_SIGNALS: &[&str] = &[
    "try again",
    "retry",
    "resets at",
    "reset in",
    "wait",
    "requests remaining",
    "periodic",
    "window",
];

const PAYLOAD_TOO_LARGE_PATTERNS: &[&str] = &[
    "request entity too large",
    "payload too large",
    "error code: 413",
];

const IMAGE_TOO_LARGE_PATTERNS: &[&str] = &[
    "image exceeds",
    "image too large",
    "image_too_large",
    "image size exceeds",
];

const CONTEXT_OVERFLOW_PATTERNS: &[&str] = &[
    "context length",
    "context size",
    "maximum context",
    "token limit",
    "too many tokens",
    "reduce the length",
    "exceeds the limit",
    "context window",
    "prompt is too long",
    "prompt exceeds max length",
    "max_tokens",
    "maximum number of tokens",
    "exceeds the max_model_len",
    "max_model_len",
    "prompt length",
    "input is too long",
    "maximum model length",
    "context length exceeded",
    "truncating input",
    "slot context",
    "n_ctx_slot",
    "max input token",
    "input token",
    "exceeds the maximum number of input tokens",
];

const MODEL_NOT_FOUND_PATTERNS: &[&str] = &[
    "is not a valid model",
    "invalid model",
    "model not found",
    "model_not_found",
    "does not exist",
    "no such model",
    "unknown model",
    "unsupported model",
];

const PROVIDER_POLICY_BLOCKED_PATTERNS: &[&str] = &[
    "no endpoints available matching your guardrail",
    "no endpoints available matching your data policy",
    "no endpoints found matching your data policy",
];

const AUTH_PATTERNS: &[&str] = &[
    "invalid api key",
    "invalid_api_key",
    "authentication",
    "unauthorized",
    "forbidden",
    "invalid token",
    "token expired",
    "token revoked",
    "access denied",
];

const TIMEOUT_MESSAGE_PATTERNS: &[&str] = &[
    "timed out",
    "turn timed out",
    "request timed out",
    "deadline exceeded",
    "operation timed out",
    "upstream timed out",
];

const TRANSPORT_ERROR_TYPES: &[&str] = &[
    "ReadTimeout",
    "ConnectTimeout",
    "PoolTimeout",
    "ConnectError",
    "RemoteProtocolError",
    "ConnectionError",
    "ConnectionResetError",
    "ConnectionAbortedError",
    "BrokenPipeError",
    "TimeoutError",
    "ReadError",
    "ServerDisconnectedError",
    "SSLError",
    "SSLZeroReturnError",
    "SSLWantReadError",
    "SSLWantWriteError",
    "SSLEOFError",
    "SSLSyscallError",
    "APIConnectionError",
    "APITimeoutError",
];

const SERVER_DISCONNECT_PATTERNS: &[&str] = &[
    "server disconnected",
    "peer closed connection",
    "connection reset by peer",
    "connection was closed",
    "network connection lost",
    "unexpected eof",
    "incomplete chunked read",
];

const SSL_TRANSIENT_PATTERNS: &[&str] = &[
    "bad record mac",
    "ssl alert",
    "tls alert",
    "ssl handshake failure",
    "tlsv1 alert",
    "sslv3 alert",
    "bad_record_mac",
    "ssl_alert",
    "tls_alert",
    "tls_alert_internal_error",
    "[ssl:",
];

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
