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
