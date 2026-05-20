use serde_json::{json, Value};

pub fn builtin_platforms() -> &'static [&'static str] {
    BUILTIN_PLATFORMS
}

pub fn parse_platform(value: &str) -> Option<&'static str> {
    let value = value.trim().to_ascii_lowercase();
    BUILTIN_PLATFORMS
        .iter()
        .copied()
        .find(|platform| *platform == value)
}

pub fn default_platform_config() -> Value {
    json!({
        "enabled": false,
        "extra": {},
        "gateway_restart_notification": true,
        "reply_to_mode": "first",
    })
}

pub fn default_session_reset_policy() -> Value {
    json!({
        "at_hour": 4,
        "idle_minutes": 1440,
        "mode": "both",
        "notify": true,
        "notify_exclude_platforms": ["api_server", "webhook"],
    })
}

pub fn home_channel(platform: &str, chat_id: &str, name: &str, thread_id: Option<&str>) -> Value {
    let mut value = json!({
        "chat_id": chat_id,
        "name": name,
        "platform": platform,
    });
    if let Some(thread_id) = thread_id {
        value["thread_id"] = json!(thread_id);
    }
    value
}

pub fn message_types() -> &'static [&'static str] {
    &[
        "text", "location", "photo", "video", "audio", "voice", "document", "sticker", "command",
    ]
}

pub fn processing_outcomes() -> &'static [&'static str] {
    &["success", "failure", "cancelled"]
}

pub fn should_send_media_as_audio(platform: &str, ext: &str, is_voice: bool) -> bool {
    let ext = ext.to_ascii_lowercase();
    let audio_exts = [".ogg", ".opus", ".mp3", ".wav", ".m4a", ".flac"];
    if !audio_exts.contains(&ext.as_str()) {
        return false;
    }
    if platform.eq_ignore_ascii_case("telegram") {
        return match ext.as_str() {
            ".ogg" | ".opus" => is_voice,
            ".mp3" | ".m4a" => true,
            _ => false,
        };
    }
    true
}

pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

pub fn safe_url_for_log(url: Option<&str>, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let Some(raw) = url else {
        return String::new();
    };
    if raw.is_empty() {
        return String::new();
    }

    let safe = if let Some((scheme, rest)) = raw.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let netloc = authority
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(authority);
        if path.is_empty() {
            format!("{scheme}://{netloc}")
        } else {
            let path_without_query = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
            let path_without_query = path_without_query
                .split_once('#')
                .map(|(path, _)| path)
                .unwrap_or(path_without_query);
            let basename = path_without_query.rsplit('/').next().unwrap_or("");
            if basename.is_empty() {
                format!("{scheme}://{netloc}/...")
            } else {
                format!("{scheme}://{netloc}/.../{basename}")
            }
        }
    } else {
        raw.to_string()
    };

    if safe.len() <= max_len {
        return safe;
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    format!("{}...", &safe[..max_len - 3])
}

pub fn thread_metadata_for_source(
    source: &SessionSource,
    reply_to_message_id: Option<&str>,
) -> Value {
    let Some(thread_id) = &source.thread_id else {
        return Value::Null;
    };
    let mut metadata = json!({"thread_id": thread_id});
    if source.platform == "telegram" && source.chat_type == "dm" {
        metadata["telegram_dm_topic_reply_fallback"] = json!(true);
        if !thread_id.is_empty() && thread_id != "1" {
            metadata["direct_messages_topic_id"] = json!(thread_id);
        }
        let anchor = reply_to_message_id.or(source.message_id.as_deref());
        if let Some(anchor) = anchor {
            metadata["telegram_reply_to_message_id"] = json!(anchor);
        }
    }
    metadata
}

pub fn reply_anchor_for_event(
    source: &SessionSource,
    message_id: Option<&str>,
    reply_to_message_id: Option<&str>,
) -> Option<String> {
    if source.platform == "telegram" && source.thread_id.is_some() && source.chat_type == "dm" {
        return message_id.or(reply_to_message_id).map(str::to_string);
    }
    if source.platform == "telegram" && source.thread_id.is_some() {
        return None;
    }
    if source.platform == "feishu" && source.thread_id.is_some() && reply_to_message_id.is_some() {
        return reply_to_message_id.map(str::to_string);
    }
    message_id.map(str::to_string)
}

pub fn webhook_is_loopback_host(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "::1" | "ip6-localhost" | "ip6-loopback"
    )
}

pub fn api_coerce_port(value: &Value, default: i64) -> i64 {
    match value {
        Value::Number(number) => number.as_i64().unwrap_or(default),
        Value::String(text) => text.parse::<i64>().unwrap_or(default),
        _ => default,
    }
}

pub fn api_coerce_request_bool(value: &Value, default: bool) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Null => default,
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Value::Number(number) => number.as_i64().unwrap_or(0) != 0,
        _ => default,
    }
}

pub fn api_normalize_chat_content(content: &Value) -> String {
    normalize_chat_content_inner(content, 0)
}

pub fn slack_extract_text_from_blocks(blocks: &Value) -> String {
    let mut parts = Vec::new();
    for block in blocks.as_array().into_iter().flatten() {
        if block.get("type").and_then(Value::as_str) == Some("rich_text") {
            walk_slack_elements(
                block.get("elements").and_then(Value::as_array),
                0,
                "",
                &mut parts,
            );
        }
    }
    parts.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSource {
    pub platform: String,
    pub chat_id: String,
    pub chat_name: Option<String>,
    pub chat_type: String,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub thread_id: Option<String>,
    pub chat_topic: Option<String>,
    pub user_id_alt: Option<String>,
    pub chat_id_alt: Option<String>,
    pub guild_id: Option<String>,
    pub parent_chat_id: Option<String>,
    pub message_id: Option<String>,
}

fn normalize_chat_content_inner(content: &Value, depth: usize) -> String {
    if depth > 10 {
        return String::new();
    }
    match content {
        Value::Null => String::new(),
        Value::String(text) => truncate_chars(text, 65_536),
        Value::Array(items) => {
            let mut parts = Vec::new();
            let mut total = 0usize;
            for item in items.iter().take(1_000) {
                let part = match item {
                    Value::String(text) => truncate_chars(text, 65_536),
                    Value::Object(obj) => match obj
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_ascii_lowercase()
                        .as_str()
                    {
                        "text" | "input_text" | "output_text" => obj
                            .get("text")
                            .map(|value| match value {
                                Value::String(text) => text.clone(),
                                other => other.to_string(),
                            })
                            .unwrap_or_default(),
                        _ => String::new(),
                    },
                    Value::Array(_) => normalize_chat_content_inner(item, depth + 1),
                    _ => String::new(),
                };
                if !part.is_empty() {
                    total += part.len();
                    parts.push(truncate_chars(&part, 65_536));
                }
                if total >= 65_536 {
                    break;
                }
            }
            truncate_chars(&parts.join("\n"), 65_536)
        }
        other => truncate_chars(
            &match other {
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => other.to_string(),
            },
            65_536,
        ),
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn walk_slack_elements(
    elements: Option<&Vec<Value>>,
    quote_depth: usize,
    bullet: &str,
    out: &mut Vec<String>,
) {
    let Some(elements) = elements else {
        return;
    };
    for element in elements {
        let element_type = element.get("type").and_then(Value::as_str).unwrap_or("");
        match element_type {
            "rich_text_section" => append_slack_line(
                &render_slack_inline_elements(element.get("elements").and_then(Value::as_array)),
                quote_depth,
                bullet,
                out,
            ),
            "rich_text_quote" => walk_slack_elements(
                element.get("elements").and_then(Value::as_array),
                quote_depth + 1,
                "",
                out,
            ),
            "rich_text_list" => {
                let ordered = element.get("style").and_then(Value::as_str) == Some("ordered");
                if let Some(items) = element.get("elements").and_then(Value::as_array) {
                    for (index, item) in items.iter().enumerate() {
                        let item_bullet = if ordered {
                            format!("{}. ", index + 1)
                        } else {
                            "\u{2022} ".to_string()
                        };
                        walk_slack_elements(
                            Some(&vec![item.clone()]),
                            quote_depth,
                            &item_bullet,
                            out,
                        );
                    }
                }
            }
            "rich_text_preformatted" => {
                let code =
                    render_slack_inline_elements(element.get("elements").and_then(Value::as_array));
                if !code.is_empty() {
                    let lang = element
                        .get("language")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    append_slack_line(&format!("```{lang}\n{code}\n```"), quote_depth, bullet, out);
                }
            }
            _ => {
                let rendered = render_slack_inline_elements(Some(&vec![element.clone()]));
                append_slack_line(&rendered, quote_depth, bullet, out);
            }
        }
    }
}

fn render_slack_inline_elements(elements: Option<&Vec<Value>>) -> String {
    let mut pieces = Vec::new();
    for element in elements.into_iter().flatten() {
        let element_type = element.get("type").and_then(Value::as_str).unwrap_or("");
        let text = match element_type {
            "text" => element
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            "link" => {
                let url = element.get("url").and_then(Value::as_str).unwrap_or("");
                let label = element.get("text").and_then(Value::as_str).unwrap_or(url);
                format!("{label} ({url})")
            }
            "channel" => format!(
                "<#{}>",
                element
                    .get("channel_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
            "user" => format!(
                "<@{}>",
                element.get("user_id").and_then(Value::as_str).unwrap_or("")
            ),
            "usergroup" => format!(
                "<!subteam^{}>",
                element
                    .get("usergroup_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
            "emoji" => format!(
                ":{}:",
                element.get("name").and_then(Value::as_str).unwrap_or("")
            ),
            "broadcast" => format!(
                "<!{}>",
                element
                    .get("range")
                    .and_then(Value::as_str)
                    .unwrap_or("here")
            ),
            "date" => element
                .get("fallback")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            "rich_text_section" => {
                render_slack_inline_elements(element.get("elements").and_then(Value::as_array))
            }
            _ => String::new(),
        };
        pieces.push(text);
    }
    pieces.join("")
}

fn append_slack_line(text: &str, quote_depth: usize, bullet: &str, out: &mut Vec<String>) {
    if text.trim().is_empty() {
        return;
    }
    let prefix = if quote_depth > 0 {
        format!("{} ", ">".repeat(quote_depth))
    } else {
        String::new()
    };
    out.push(format!("{prefix}{bullet}{}", text.trim_end()));
}

const BUILTIN_PLATFORMS: &[&str] = &[
    "local",
    "telegram",
    "discord",
    "whatsapp",
    "slack",
    "signal",
    "mattermost",
    "matrix",
    "homeassistant",
    "email",
    "sms",
    "dingtalk",
    "api_server",
    "webhook",
    "msgraph_webhook",
    "feishu",
    "wecom",
    "wecom_callback",
    "weixin",
    "bluebubbles",
    "qqbot",
    "yuanbao",
];

impl SessionSource {
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "SessionSource must be a JSON object".to_string())?;
        let required_str = |key: &str| -> Result<String, String> {
            object
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("SessionSource.{key} is required"))
        };
        let optional_str = |key: &str| -> Option<String> {
            object.get(key).and_then(Value::as_str).map(str::to_string)
        };
        Ok(Self {
            platform: required_str("platform")?,
            chat_id: required_str("chat_id")?,
            chat_name: optional_str("chat_name"),
            chat_type: optional_str("chat_type").unwrap_or_else(|| "dm".to_string()),
            user_id: optional_str("user_id"),
            user_name: optional_str("user_name"),
            thread_id: optional_str("thread_id"),
            chat_topic: optional_str("chat_topic"),
            user_id_alt: optional_str("user_id_alt"),
            chat_id_alt: optional_str("chat_id_alt"),
            guild_id: optional_str("guild_id"),
            parent_chat_id: optional_str("parent_chat_id"),
            message_id: optional_str("message_id"),
        })
    }

    pub fn to_json(&self) -> Value {
        let mut value = json!({
            "chat_id": self.chat_id,
            "chat_name": self.chat_name,
            "chat_topic": self.chat_topic,
            "chat_type": self.chat_type,
            "platform": self.platform,
            "thread_id": self.thread_id,
            "user_id": self.user_id,
            "user_name": self.user_name,
        });
        if let Some(user_id_alt) = &self.user_id_alt {
            value["user_id_alt"] = json!(user_id_alt);
        }
        if let Some(chat_id_alt) = &self.chat_id_alt {
            value["chat_id_alt"] = json!(chat_id_alt);
        }
        if let Some(guild_id) = &self.guild_id {
            value["guild_id"] = json!(guild_id);
        }
        if let Some(parent_chat_id) = &self.parent_chat_id {
            value["parent_chat_id"] = json!(parent_chat_id);
        }
        if let Some(message_id) = &self.message_id {
            value["message_id"] = json!(message_id);
        }
        value
    }

    pub fn description(&self) -> String {
        if self.platform == "local" {
            return "CLI terminal".to_string();
        }
        let mut parts = Vec::new();
        if self.chat_type == "dm" {
            parts.push(format!(
                "DM with {}",
                self.user_name
                    .as_deref()
                    .or(self.user_id.as_deref())
                    .unwrap_or("user")
            ));
        } else if self.chat_type == "group" {
            parts.push(format!(
                "group: {}",
                self.chat_name.as_deref().unwrap_or(&self.chat_id)
            ));
        } else if self.chat_type == "channel" {
            parts.push(format!(
                "channel: {}",
                self.chat_name.as_deref().unwrap_or(&self.chat_id)
            ));
        } else {
            parts.push(
                self.chat_name
                    .clone()
                    .unwrap_or_else(|| self.chat_id.clone()),
            );
        }
        if let Some(thread_id) = &self.thread_id {
            parts.push(format!("thread: {thread_id}"));
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEvent {
    pub text: String,
    pub source: SessionSource,
}

impl MessageEvent {
    pub fn is_command(&self) -> bool {
        self.text.starts_with('/')
    }

    pub fn command(&self) -> Option<String> {
        if !self.is_command() {
            return None;
        }
        let raw = self
            .text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/')
            .to_lowercase();
        let raw = raw.split('@').next().unwrap_or("").to_string();
        if raw.is_empty() || raw.contains('/') {
            None
        } else {
            Some(raw)
        }
    }

    pub fn command_args(&self) -> String {
        if !self.is_command() {
            return self.text.clone();
        }
        let args = self
            .text
            .split_once(char::is_whitespace)
            .map(|(_, rest)| rest)
            .unwrap_or("");
        args.replace("\u{2014}\u{2014}", "--")
            .replace('\u{2014}', "--")
            .replace('\u{2013}', "-")
    }
}

pub fn coerce_plaintext_gateway_command(text: &str, message_type: &str, chat_type: &str) -> String {
    if message_type != "text" || chat_type != "dm" {
        return text.to_string();
    }
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return text.to_string();
    }

    let normalized = trimmed
        .trim_end_matches(|ch: char| ch == '.' || ch == '!' || ch == '?' || ch.is_whitespace())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    let restart = matches!(
        normalized.as_str(),
        "restart gateway"
            | "please restart gateway"
            | "restart the gateway"
            | "please restart the gateway"
            | "restart hermes gateway"
            | "please restart hermes gateway"
            | "restart the hermes gateway"
            | "please restart the hermes gateway"
            | "restart hermes"
            | "please restart hermes"
    );

    if restart {
        "/restart".to_string()
    } else {
        text.to_string()
    }
}

pub fn is_shared_multi_user_session(
    source: &SessionSource,
    group_sessions_per_user: bool,
    thread_sessions_per_user: bool,
) -> bool {
    if source.chat_type == "dm" {
        return false;
    }
    if source.thread_id.is_some() {
        return !thread_sessions_per_user;
    }
    !group_sessions_per_user
}

pub fn build_session_key(
    source: &SessionSource,
    group_sessions_per_user: bool,
    thread_sessions_per_user: bool,
) -> String {
    let platform = source.platform.as_str();
    if source.chat_type == "dm" {
        let dm_chat_id = if platform == "whatsapp" {
            canonical_whatsapp_identifier(&source.chat_id)
        } else {
            source.chat_id.clone()
        };
        if !dm_chat_id.is_empty() {
            if let Some(thread_id) = &source.thread_id {
                return format!("agent:main:{platform}:dm:{dm_chat_id}:{thread_id}");
            }
            return format!("agent:main:{platform}:dm:{dm_chat_id}");
        }
        if let Some(thread_id) = &source.thread_id {
            return format!("agent:main:{platform}:dm:{thread_id}");
        }
        return format!("agent:main:{platform}:dm");
    }

    let mut participant_id = source
        .user_id_alt
        .as_deref()
        .or(source.user_id.as_deref())
        .map(str::to_string);
    if platform == "whatsapp" {
        if let Some(participant) = participant_id.as_deref() {
            let canonical = canonical_whatsapp_identifier(participant);
            if !canonical.is_empty() {
                participant_id = Some(canonical);
            }
        }
    }

    let mut key_parts = vec![
        "agent".to_string(),
        "main".to_string(),
        platform.to_string(),
        source.chat_type.clone(),
    ];
    if !source.chat_id.is_empty() {
        key_parts.push(source.chat_id.clone());
    }
    if let Some(thread_id) = &source.thread_id {
        key_parts.push(thread_id.clone());
    }

    let mut isolate_user = group_sessions_per_user;
    if source.thread_id.is_some() && !thread_sessions_per_user {
        isolate_user = false;
    }
    if isolate_user {
        if let Some(participant_id) = participant_id {
            key_parts.push(participant_id);
        }
    }
    key_parts.join(":")
}

fn canonical_whatsapp_identifier(value: &str) -> String {
    value
        .trim()
        .strip_prefix('+')
        .unwrap_or_else(|| value.trim())
        .split_once(':')
        .map(|(head, _)| head)
        .unwrap_or_else(|| {
            value
                .trim()
                .strip_prefix('+')
                .unwrap_or_else(|| value.trim())
        })
        .split_once('@')
        .map(|(head, _)| head)
        .unwrap_or_else(|| {
            value
                .trim()
                .strip_prefix('+')
                .unwrap_or_else(|| value.trim())
                .split_once(':')
                .map(|(head, _)| head)
                .unwrap_or_else(|| {
                    value
                        .trim()
                        .strip_prefix('+')
                        .unwrap_or_else(|| value.trim())
                })
        })
        .to_string()
}
