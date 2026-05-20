use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretMetadata {
    pub category: Option<&'static str>,
    pub password: Option<bool>,
}

#[derive(Debug)]
pub enum ConfigError {
    ParseYaml(serde_yaml::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ParseYaml(err) => write!(f, "failed to parse config YAML: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn load_config_from_yaml(
    defaults: &Value,
    user_yaml: &str,
    default_max_turns: i64,
) -> Result<Value, ConfigError> {
    let overlay = if user_yaml.trim().is_empty() {
        json!({})
    } else {
        serde_yaml::from_str::<Value>(user_yaml).map_err(ConfigError::ParseYaml)?
    };
    let normalized = normalize_max_turns(normalize_root_model_keys(overlay), default_max_turns);
    Ok(deep_merge(defaults, &normalized))
}

pub fn default_top_level_keys() -> &'static [&'static str] {
    DEFAULT_TOP_LEVEL_KEYS
}

pub fn selected_default_values() -> Value {
    json!({
        "_config_version": 23,
        "agent.api_max_retries": 3,
        "agent.disabled_toolsets": [],
        "agent.gateway_timeout": 1800,
        "agent.image_input_mode": "auto",
        "agent.max_turns": 90,
        "agent.restart_drain_timeout": 180,
        "browser.allow_private_urls": false,
        "browser.command_timeout": 30,
        "browser.engine": "auto",
        "browser.inactivity_timeout": 120,
        "checkpoints.enabled": false,
        "cron.wrap_response": true,
        "display.busy_input_mode": "interrupt",
        "display.skin": "default",
        "logging.level": "INFO",
        "lsp.enabled": true,
        "memory.memory_enabled": true,
        "memory.provider": "",
        "model": "",
        "security.allow_lazy_installs": true,
        "security.redact_secrets": true,
        "sessions.auto_prune": false,
        "terminal.backend": "local",
        "terminal.container_cpu": 1,
        "terminal.container_memory": 5120,
        "terminal.container_persistent": true,
        "terminal.cwd": ".",
        "terminal.docker_image": "nikolaik/python-nodejs:python3.11-nodejs20",
        "terminal.timeout": 180,
        "toolsets": ["hermes-cli"],
        "updates.pre_update_backup": false,
    })
}

pub fn recommended_update_command_for_method(method: &str) -> &'static str {
    match method {
        "nixos" => "Update your Nix flake input and rebuild (e.g. nix flake update, nixos-rebuild, or home-manager switch)",
        "homebrew" => "brew upgrade hermes-agent",
        "docker" => "docker pull nousresearch/hermes-agent:latest",
        "pip" => "pip install --upgrade hermes-agent",
        _ => "hermes update",
    }
}

pub fn install_method_stamp(method: &str) -> String {
    format!("{method}\n")
}

pub fn detect_install_method_from_stamp(stamp: &str) -> Option<String> {
    let method = stamp.trim().to_ascii_lowercase();
    (!method.is_empty()).then_some(method)
}

pub fn durable_profile_paths() -> &'static [&'static str] {
    DURABLE_PROFILE_PATHS
}

pub fn reloadable_profile_paths() -> &'static [&'static str] {
    RELOADABLE_PROFILE_PATHS
}

pub fn migration_dry_run_report(home: &Path) -> Value {
    let durable = path_statuses(home, DURABLE_PROFILE_PATHS);
    let reloadable = path_statuses(home, RELOADABLE_PROFILE_PATHS);
    let durable_existing = existing_relative_paths(&durable);
    json!({
        "backup_required": durable_existing,
        "durable": durable,
        "reloadable": reloadable,
        "report_redacted": true,
        "would_delete": [],
        "would_write": [],
    })
}

pub fn profile_migration_constants_fixture() -> Value {
    json!({
        "profile_dirs": PROFILE_DIRS,
        "clone_config_files": CLONE_CONFIG_FILES,
        "clone_subdir_files": CLONE_SUBDIR_FILES,
        "clone_all_strip": CLONE_ALL_STRIP,
        "clone_all_default_exclude_root": sorted_strings(CLONE_ALL_DEFAULT_EXCLUDE_ROOT),
        "default_export_exclude_root": sorted_strings(DEFAULT_EXPORT_EXCLUDE_ROOT),
        "no_bundled_skills_marker": NO_BUNDLED_SKILLS_MARKER,
    })
}

pub fn profile_name_validation_cases(inputs: &[&str]) -> Value {
    Value::Array(
        inputs
            .iter()
            .map(|input| match normalize_profile_name(input) {
                Ok(normalized) => match validate_profile_name(&normalized) {
                    Ok(()) => json!({
                        "input": input,
                        "normalized": normalized,
                        "valid_after_normalize": true,
                        "validation_error": null,
                    }),
                    Err(error) => json!({
                        "input": input,
                        "normalized": normalized,
                        "valid_after_normalize": false,
                        "validation_error": error,
                    }),
                },
                Err(error) => json!({
                    "input": input,
                    "normalized": null,
                    "valid_after_normalize": false,
                    "validation_error": error,
                }),
            })
            .collect(),
    )
}

pub fn clone_all_ignore(
    names: &[&str],
    is_default_source_root: bool,
    at_source_root: bool,
) -> Value {
    let ignored = names
        .iter()
        .copied()
        .filter(|entry| {
            is_universal_clone_exclude(entry)
                || (is_default_source_root
                    && at_source_root
                    && CLONE_ALL_DEFAULT_EXCLUDE_ROOT.contains(entry))
        })
        .collect::<Vec<_>>();
    json!(sorted_strings(&ignored))
}

pub fn export_ignore(names: &[&str], at_root: bool) -> Value {
    let ignored = names
        .iter()
        .copied()
        .filter(|entry| {
            is_universal_export_exclude(entry)
                || (at_root && DEFAULT_EXPORT_EXCLUDE_ROOT.contains(entry))
        })
        .collect::<Vec<_>>();
    json!(sorted_strings(&ignored))
}

pub fn deep_merge(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_obj), Value::Object(overlay_obj)) => {
            let mut merged = base_obj.clone();
            for (key, value) in overlay_obj {
                let next = merged
                    .get(key)
                    .map(|base_value| deep_merge(base_value, value))
                    .unwrap_or_else(|| value.clone());
                merged.insert(key.clone(), next);
            }
            Value::Object(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}

const DEFAULT_TOP_LEVEL_KEYS: &[&str] = &[
    "_config_version",
    "agent",
    "approvals",
    "auxiliary",
    "bedrock",
    "browser",
    "checkpoints",
    "code_execution",
    "command_allowlist",
    "compression",
    "context",
    "credential_pool_strategies",
    "cron",
    "curator",
    "dashboard",
    "delegation",
    "discord",
    "display",
    "fallback_providers",
    "file_read_max_chars",
    "goals",
    "honcho",
    "hooks",
    "hooks_auto_accept",
    "human_delay",
    "kanban",
    "logging",
    "lsp",
    "matrix",
    "mattermost",
    "memory",
    "model",
    "model_catalog",
    "network",
    "onboarding",
    "openrouter",
    "personalities",
    "prefill_messages_file",
    "privacy",
    "prompt_caching",
    "providers",
    "quick_commands",
    "security",
    "sessions",
    "skills",
    "slack",
    "stt",
    "telegram",
    "terminal",
    "timezone",
    "tool_loop_guardrails",
    "tool_output",
    "toolsets",
    "tts",
    "updates",
    "voice",
    "web",
    "whatsapp",
    "x_search",
];

const DURABLE_PROFILE_PATHS: &[&str] = &[
    "config.yaml",
    ".env",
    "state.db",
    "state.db-wal",
    "state.db-shm",
    "sessions",
    "skills",
    "plugins",
    "cron",
    "gateway",
    "memories/MEMORY.md",
    "memories/USER.md",
    "tool_history",
    "checkpoints",
    "trajectories",
    "exports",
    "logs",
];

const RELOADABLE_PROFILE_PATHS: &[&str] = &[
    "bin/hermes",
    "completions",
    "default-config",
    "bundled-skills",
    "optional-skills-index",
    "docs",
    "target",
];

const PROFILE_DIRS: &[&str] = &[
    "memories",
    "sessions",
    "skills",
    "skins",
    "logs",
    "plans",
    "workspace",
    "cron",
    "home",
];

const CLONE_CONFIG_FILES: &[&str] = &["config.yaml", ".env", "SOUL.md"];
const CLONE_SUBDIR_FILES: &[&str] = &["memories/MEMORY.md", "memories/USER.md"];
const CLONE_ALL_STRIP: &[&str] = &["gateway.pid", "gateway_state.json", "processes.json"];
const CLONE_ALL_DEFAULT_EXCLUDE_ROOT: &[&str] = &[
    "hermes-agent",
    ".worktrees",
    "profiles",
    "bin",
    "node_modules",
];
const NO_BUNDLED_SKILLS_MARKER: &str = ".no-bundled-skills";
const RESERVED_PROFILE_NAMES: &[&str] = &["hermes", "default", "test", "tmp", "root", "sudo"];
const DEFAULT_EXPORT_EXCLUDE_ROOT: &[&str] = &[
    "hermes-agent",
    ".worktrees",
    "profiles",
    "bin",
    "node_modules",
    "state.db",
    "state.db-shm",
    "state.db-wal",
    "hermes_state.db",
    "response_store.db",
    "response_store.db-shm",
    "response_store.db-wal",
    "gateway.pid",
    "gateway_state.json",
    "processes.json",
    "auth.json",
    ".env",
    "auth.lock",
    "active_profile",
    ".update_check",
    "errors.log",
    ".hermes_history",
    "image_cache",
    "audio_cache",
    "document_cache",
    "browser_screenshots",
    "checkpoints",
    "sandboxes",
    "logs",
];

fn path_statuses(home: &Path, paths: &[&str]) -> Value {
    Value::Array(
        paths
            .iter()
            .map(|relative| {
                let kind = match fs::metadata(home.join(relative)) {
                    Ok(metadata) if metadata.is_dir() => "dir",
                    Ok(metadata) if metadata.is_file() => "file",
                    Ok(_) => "other",
                    Err(_) => "missing",
                };
                json!({
                    "kind": kind,
                    "path": relative,
                    "present": kind != "missing",
                })
            })
            .collect(),
    )
}

fn existing_relative_paths(statuses: &Value) -> Value {
    let Some(entries) = statuses.as_array() else {
        return Value::Array(Vec::new());
    };
    Value::Array(
        entries
            .iter()
            .filter(|entry| entry["present"].as_bool().unwrap_or(false))
            .map(|entry| entry["path"].clone())
            .collect(),
    )
}

fn normalize_profile_name(name: &str) -> Result<String, String> {
    let stripped = name.trim();
    if stripped.is_empty() {
        return Err("profile name cannot be empty".to_string());
    }
    if stripped.eq_ignore_ascii_case("default") {
        return Ok("default".to_string());
    }
    Ok(stripped.to_ascii_lowercase())
}

fn validate_profile_name(name: &str) -> Result<(), String> {
    if name == "default" {
        return Ok(());
    }
    if !valid_profile_id(name) {
        return Err(format!(
            "Invalid profile name '{}'. Must match [a-z0-9][a-z0-9_-]{{0,63}}",
            name.replace('\'', "\\'")
        ));
    }
    if RESERVED_PROFILE_NAMES.contains(&name) {
        return Err(format!(
            "Profile name '{}' is reserved — it collides with either the Hermes installation itself or a common system binary.  Pick a different name.",
            name.replace('\'', "\\'")
        ));
    }
    Ok(())
}

fn valid_profile_id(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    let first = bytes[0];
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    bytes.iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    })
}

fn is_universal_clone_exclude(entry: &str) -> bool {
    entry == "__pycache__"
        || entry.ends_with(".pyc")
        || entry.ends_with(".pyo")
        || entry.ends_with(".sock")
        || entry.ends_with(".tmp")
}

fn is_universal_export_exclude(entry: &str) -> bool {
    entry == "__pycache__"
        || entry.ends_with(".sock")
        || entry.ends_with(".tmp")
        || matches!(entry, "package.json" | "package-lock.json")
}

fn sorted_strings(values: &[&str]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    values.sort();
    values
}

pub fn set_dot_path(config: &mut Value, path: &str, value: Value) {
    let parts = path
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return;
    }

    if !config.is_object() {
        *config = json!({});
    }

    let mut cursor = config;
    for part in &parts[..parts.len() - 1] {
        if !cursor.get(part).is_some_and(Value::is_object) {
            cursor[part] = json!({});
        }
        cursor = cursor.get_mut(part).expect("path segment just inserted");
    }
    cursor[parts[parts.len() - 1]] = value;
}

pub fn parse_config_set_value(value: &str) -> Value {
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "true" | "yes" | "on") {
        return json!(true);
    }
    if matches!(lower.as_str(), "false" | "no" | "off") {
        return json!(false);
    }
    if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
        if let Ok(parsed) = value.parse::<i64>() {
            return json!(parsed);
        }
    }
    if value.matches('.').count() == 1
        && value.replace('.', "").chars().all(|ch| ch.is_ascii_digit())
    {
        if let Ok(parsed) = value.parse::<f64>() {
            return json!(parsed);
        }
    }
    json!(value)
}

pub fn set_nested_path(config: &mut Value, dotted_key: &str, value: Value) -> Result<(), String> {
    let parts = dotted_key
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(());
    }

    if !matches!(config, Value::Object(_) | Value::Array(_)) {
        *config = json!({});
    }

    let mut current = config;
    for part in &parts[..parts.len() - 1] {
        match current {
            Value::Array(items) => {
                let index = part.parse::<usize>().map_err(|_| {
                    format!(
                        "Cannot navigate into list at key {dotted_key:?}: segment {part:?} is not a numeric index"
                    )
                })?;
                current = items.get_mut(index).ok_or_else(|| {
                    format!("list index {index} out of range for key {dotted_key:?}")
                })?;
            }
            Value::Object(object) => {
                let needs_insert = !matches!(
                    object.get(*part),
                    Some(Value::Object(_)) | Some(Value::Array(_))
                );
                if needs_insert {
                    object.insert((*part).to_string(), json!({}));
                }
                current = object
                    .get_mut(*part)
                    .expect("path segment just inserted or existed");
            }
            other => {
                return Err(format!(
                    "Cannot navigate into {} at key {dotted_key:?}",
                    value_type_name(other)
                ));
            }
        }
    }

    let last = parts[parts.len() - 1];
    match current {
        Value::Array(items) => {
            let index = last.parse::<usize>().map_err(|_| {
                format!(
                    "Cannot navigate into list at key {dotted_key:?}: segment {last:?} is not a numeric index"
                )
            })?;
            let slot = items
                .get_mut(index)
                .ok_or_else(|| format!("list index {index} out of range for key {dotted_key:?}"))?;
            *slot = value;
        }
        Value::Object(object) => {
            object.insert(last.to_string(), value);
        }
        other => {
            return Err(format!(
                "Cannot navigate into {} at key {dotted_key:?}",
                value_type_name(other)
            ));
        }
    }
    Ok(())
}

pub fn terminal_env_sync_key(config_key: &str) -> Option<&'static str> {
    match config_key {
        "terminal.backend" => Some("TERMINAL_ENV"),
        "terminal.modal_mode" => Some("TERMINAL_MODAL_MODE"),
        "terminal.docker_image" => Some("TERMINAL_DOCKER_IMAGE"),
        "terminal.singularity_image" => Some("TERMINAL_SINGULARITY_IMAGE"),
        "terminal.modal_image" => Some("TERMINAL_MODAL_IMAGE"),
        "terminal.daytona_image" => Some("TERMINAL_DAYTONA_IMAGE"),
        "terminal.vercel_runtime" => Some("TERMINAL_VERCEL_RUNTIME"),
        "terminal.docker_mount_cwd_to_workspace" => Some("TERMINAL_DOCKER_MOUNT_CWD_TO_WORKSPACE"),
        "terminal.docker_run_as_host_user" => Some("TERMINAL_DOCKER_RUN_AS_HOST_USER"),
        "terminal.docker_env" => Some("TERMINAL_DOCKER_ENV"),
        "terminal.timeout" => Some("TERMINAL_TIMEOUT"),
        "terminal.sandbox_dir" => Some("TERMINAL_SANDBOX_DIR"),
        "terminal.persistent_shell" => Some("TERMINAL_PERSISTENT_SHELL"),
        "terminal.container_cpu" => Some("TERMINAL_CONTAINER_CPU"),
        "terminal.container_memory" => Some("TERMINAL_CONTAINER_MEMORY"),
        "terminal.container_disk" => Some("TERMINAL_CONTAINER_DISK"),
        "terminal.container_persistent" => Some("TERMINAL_CONTAINER_PERSISTENT"),
        _ => None,
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

pub fn normalize_root_model_keys(mut config: Value) -> Value {
    let Some(obj) = config.as_object_mut() else {
        return config;
    };

    let root_provider = obj.get("provider").cloned().filter(non_empty);
    let root_base_url = obj.get("base_url").cloned().filter(non_empty);
    let root_context_length = obj.get("context_length").cloned().filter(non_empty);

    if root_provider.is_none() && root_base_url.is_none() && root_context_length.is_none() {
        return config;
    }

    if !matches!(obj.get("model"), Some(Value::Object(_))) {
        let existing = obj.get("model").cloned().filter(non_empty);
        let mut model = Map::new();
        if let Some(value) = existing {
            model.insert("default".to_string(), value);
        }
        obj.insert("model".to_string(), Value::Object(model));
    }

    let model = obj
        .get_mut("model")
        .and_then(Value::as_object_mut)
        .expect("model object just inserted");

    insert_if_empty(model, "provider", root_provider);
    insert_if_empty(model, "base_url", root_base_url);
    insert_if_empty(model, "context_length", root_context_length);

    obj.remove("provider");
    obj.remove("base_url");
    obj.remove("context_length");
    config
}

pub fn normalize_max_turns(mut config: Value, default_max_turns: i64) -> Value {
    let Some(obj) = config.as_object_mut() else {
        return config;
    };

    let root_max_turns = obj.get("max_turns").cloned();
    if !matches!(obj.get("agent"), Some(Value::Object(_))) {
        obj.insert("agent".to_string(), json!({}));
    }

    let agent = obj
        .get_mut("agent")
        .and_then(Value::as_object_mut)
        .expect("agent object just inserted");

    if !agent.contains_key("max_turns") || agent.get("max_turns").is_some_and(Value::is_null) {
        agent.insert(
            "max_turns".to_string(),
            root_max_turns.unwrap_or_else(|| json!(default_max_turns)),
        );
    }

    obj.remove("max_turns");
    config
}

pub fn sanitize_env_lines(lines: &[&str], known_keys: &[&str]) -> Vec<String> {
    let known: BTreeSet<&str> = known_keys.iter().copied().collect();
    let mut sanitized = Vec::new();

    for raw_line in lines {
        let stripped = raw_line.trim().trim_end_matches(['\r', '\n']);
        if stripped.is_empty() || stripped.starts_with('#') {
            sanitized.push(stripped.to_string());
            continue;
        }

        let mut positions = Vec::new();
        for key in &known {
            let needle = format!("{key}=");
            let mut start = 0;
            while let Some(offset) = stripped[start..].find(&needle) {
                let pos = start + offset;
                positions.push((pos, pos + needle.len()));
                start = pos + needle.len();
            }
        }

        let mut split_positions: Vec<usize> = positions
            .iter()
            .filter_map(|(start, end)| {
                let contained = positions.iter().any(|(other_start, other_end)| {
                    (other_start, other_end) != (start, end)
                        && other_start <= start
                        && other_end >= end
                });
                (!contained).then_some(*start)
            })
            .collect();
        split_positions.sort_unstable();
        split_positions.dedup();

        if split_positions.len() > 1 {
            for (index, pos) in split_positions.iter().enumerate() {
                let end = split_positions
                    .get(index + 1)
                    .copied()
                    .unwrap_or(stripped.len());
                let part = stripped[*pos..end].trim();
                if !part.is_empty() {
                    sanitized.push(part.to_string());
                }
            }
        } else {
            sanitized.push(stripped.to_string());
        }
    }

    sanitized
}

pub fn parse_env_lines(lines: &[String]) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            env.insert(
                key.trim().to_string(),
                value.trim().trim_matches(['"', '\'']).to_string(),
            );
        }
    }
    env
}

pub fn known_secret_metadata(key: &str) -> SecretMetadata {
    match key {
        "ANTHROPIC_API_KEY" | "OPENROUTER_API_KEY" => SecretMetadata {
            category: Some("provider"),
            password: Some(true),
        },
        "OPENAI_API_KEY" => SecretMetadata {
            category: None,
            password: None,
        },
        _ => SecretMetadata {
            category: None,
            password: None,
        },
    }
}

pub fn discover_env_values(env_text: &str, known_keys: &[&str]) -> BTreeMap<String, String> {
    let lines = env_text.lines().map(str::to_string).collect::<Vec<_>>();
    let parsed = parse_env_lines(&lines);
    known_keys
        .iter()
        .filter_map(|key| {
            parsed
                .get(*key)
                .map(|value| ((*key).to_string(), value.clone()))
        })
        .collect()
}

pub fn redact_key(value: &str) -> String {
    if value.is_empty() {
        return "(not set)".to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "***".to_string();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}...{tail}")
}

pub fn mask_secret(value: &str, empty: &str) -> String {
    if value.is_empty() {
        return empty.to_string();
    }
    if value.chars().count() < 12 {
        return "***".to_string();
    }
    mask_with(value, 4, 4)
}

pub fn redact_sensitive_text(text: &str, code_file: bool) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----") {
        return "[REDACTED PRIVATE KEY]".to_string();
    }

    let mut out = text.to_string();
    out = redact_db_url(&out);
    out = redact_url_userinfo(&out);
    out = redact_url_query(&out);
    out = redact_form_body(&out);
    out = redact_discord_mentions(&out);
    out = redact_phone_numbers(&out);

    if !code_file {
        out = redact_env_assignment(&out);
        out = redact_json_secret_field(&out);
    }

    out = redact_authorization_bearer(&out);
    redact_prefixed_tokens(&out)
}

fn mask_log_token(value: &str) -> String {
    if value.is_empty() || value.chars().count() < 18 {
        return "***".to_string();
    }
    mask_with(value, 6, 4)
}

fn mask_with(value: &str, head: usize, tail: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let prefix = chars.iter().take(head).collect::<String>();
    let suffix = chars[chars.len().saturating_sub(tail)..]
        .iter()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn redact_prefixed_tokens(text: &str) -> String {
    text.split_whitespace()
        .map(|token| {
            let trimmed = token.trim_matches(|ch: char| {
                matches!(ch, '"' | '\'' | ',' | ';' | ')' | '(' | '[' | ']')
            });
            if is_known_token_prefix(trimmed) {
                token.replace(trimmed, &mask_log_token(trimmed))
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_known_token_prefix(value: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "sk-",
        "ghp_",
        "github_pat_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "xoxb-",
        "xoxa-",
        "xoxp-",
        "xoxr-",
        "xoxs-",
        "pplx-",
        "fal_",
        "fc-",
        "bb_live_",
        "gAAAA",
        "sk_live_",
        "sk_test_",
        "rk_live_",
        "SG.",
        "hf_",
        "r8_",
        "npm_",
        "pypi-",
        "dop_v1_",
        "doo_v1_",
        "am_",
        "sk_",
        "tvly-",
        "exa_",
        "gsk_",
        "syt_",
        "retaindb_",
        "hsk-",
        "mem0_",
        "brv_",
        "xai-",
    ];
    PREFIXES.iter().any(|prefix| value.starts_with(prefix)) && value.len() >= 10
}

fn redact_authorization_bearer(text: &str) -> String {
    const NEEDLE: &str = "Authorization: Bearer ";
    if let Some(start) = text.to_ascii_lowercase().find(&NEEDLE.to_ascii_lowercase()) {
        let value_start = start + NEEDLE.len();
        let value_end = text[value_start..]
            .find(char::is_whitespace)
            .map(|offset| value_start + offset)
            .unwrap_or(text.len());
        let mut out = String::new();
        out.push_str(&text[..value_start]);
        out.push_str(&mask_log_token(&text[value_start..value_end]));
        out.push_str(&text[value_end..]);
        out
    } else {
        text.to_string()
    }
}

fn redact_env_assignment(text: &str) -> String {
    let Some((key, value)) = text.split_once('=') else {
        return text.to_string();
    };
    if key.chars().any(|ch| ch.is_ascii_lowercase()) || text.contains('&') {
        return text.to_string();
    }
    let upper = key.to_ascii_uppercase();
    if [
        "API_KEY",
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "CREDENTIAL",
        "AUTH",
    ]
    .iter()
    .any(|needle| upper.contains(needle))
    {
        let _ = value;
        format!("{key}=***")
    } else {
        text.to_string()
    }
}

fn redact_json_secret_field(text: &str) -> String {
    let Some((prefix, rest)) = text.split_once(": \"") else {
        return text.to_string();
    };
    let key = prefix
        .trim()
        .trim_start_matches('{')
        .trim()
        .trim_matches('"')
        .to_ascii_lowercase();
    let sensitive = matches!(
        key.as_str(),
        "api_key"
            | "apikey"
            | "token"
            | "secret"
            | "password"
            | "access_token"
            | "refresh_token"
            | "auth_token"
            | "bearer"
            | "secret_value"
            | "raw_secret"
            | "secret_input"
            | "key_material"
    );
    if !sensitive {
        return text.to_string();
    }
    let Some((_value, suffix)) = rest.split_once('"') else {
        return text.to_string();
    };
    format!("{prefix}: \"***\"{suffix}")
}

fn redact_url_query(text: &str) -> String {
    let Some((before, after_question)) = text.split_once('?') else {
        return text.to_string();
    };
    if !before.contains("://") {
        return text.to_string();
    }
    let (query, fragment) = after_question
        .split_once('#')
        .map_or((after_question, ""), |(query, fragment)| (query, fragment));
    let redacted = redact_query_pairs(query);
    if fragment.is_empty() {
        format!("{before}?{redacted}")
    } else {
        format!("{before}?{redacted}#{fragment}")
    }
}

fn redact_form_body(text: &str) -> String {
    if text.contains('\n') || !text.contains('&') || text.contains("://") {
        return text.to_string();
    }
    if text.split('&').all(|pair| pair.contains('=')) {
        redact_query_pairs(text)
    } else {
        text.to_string()
    }
}

fn redact_query_pairs(query: &str) -> String {
    query
        .split('&')
        .map(|pair| {
            let Some((key, value)) = pair.split_once('=') else {
                return pair.to_string();
            };
            if is_sensitive_param(key) {
                format!("{key}=***")
            } else {
                format!("{key}={value}")
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn is_sensitive_param(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "access_token"
            | "refresh_token"
            | "id_token"
            | "token"
            | "api_key"
            | "apikey"
            | "client_secret"
            | "password"
            | "auth"
            | "jwt"
            | "session"
            | "secret"
            | "key"
            | "code"
            | "signature"
            | "x-amz-signature"
    )
}

fn redact_db_url(text: &str) -> String {
    for scheme in [
        "postgres://",
        "postgresql://",
        "mysql://",
        "mongodb://",
        "redis://",
        "amqp://",
    ] {
        if let Some(start) = text.to_ascii_lowercase().find(scheme) {
            if let Some(colon) = text[start + scheme.len()..].find(':') {
                let pass_start = start + scheme.len() + colon + 1;
                if let Some(at) = text[pass_start..].find('@') {
                    let pass_end = pass_start + at;
                    return format!("{}***{}", &text[..pass_start], &text[pass_end..]);
                }
            }
        }
    }
    text.to_string()
}

fn redact_url_userinfo(text: &str) -> String {
    for scheme in ["https://", "http://", "wss://", "ws://", "ftp://"] {
        if let Some(start) = text.to_ascii_lowercase().find(scheme) {
            if let Some(colon) = text[start + scheme.len()..].find(':') {
                let pass_start = start + scheme.len() + colon + 1;
                if let Some(at) = text[pass_start..].find('@') {
                    let pass_end = pass_start + at;
                    return format!("{}***{}", &text[..pass_start], &text[pass_end..]);
                }
            }
        }
    }
    text.to_string()
}

fn redact_discord_mentions(text: &str) -> String {
    let Some(start) = text.find("<@") else {
        return text.to_string();
    };
    let Some(end_offset) = text[start..].find('>') else {
        return text.to_string();
    };
    let end = start + end_offset;
    let inner = &text[start + 2..end];
    let snowflake = inner
        .strip_prefix('!')
        .unwrap_or(inner)
        .chars()
        .all(|ch| ch.is_ascii_digit())
        && inner.strip_prefix('!').unwrap_or(inner).len() >= 17;
    if snowflake {
        format!(
            "{}<@{}***>{}",
            &text[..start],
            if inner.starts_with('!') { "!" } else { "" },
            &text[end + 1..]
        )
    } else {
        text.to_string()
    }
}

fn redact_phone_numbers(text: &str) -> String {
    for token in text.split_whitespace() {
        if token.starts_with('+')
            && token[1..].chars().all(|ch| ch.is_ascii_digit())
            && (7..=15).contains(&token[1..].len())
        {
            let replacement = if token.len() <= 8 {
                format!("{}****{}", &token[..2], &token[token.len() - 2..])
            } else {
                format!("{}****{}", &token[..4], &token[token.len() - 4..])
            };
            return text.replacen(token, &replacement, 1);
        }
    }
    text.to_string()
}

fn non_empty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.is_empty(),
        _ => true,
    }
}

fn insert_if_empty(model: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        let should_insert = model.get(key).is_none_or(|existing| !non_empty(existing));
        if should_insert {
            model.insert(key.to_string(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Value {
        json!({
            "_config_version": 23,
            "agent": {"max_turns": 90, "system_prompt": null},
            "display": {"skin": "default", "tool_progress_command": false},
            "memory": {"provider": "", "enabled": null},
            "model": {"raw": null, "default": null, "provider": null, "base_url": null},
            "terminal": {"cwd": ".", "backend": "local"},
        })
    }

    #[test]
    fn loading_yaml_preserves_unknown_user_sections() {
        let config = load_config_from_yaml(
            &defaults(),
            r#"
model:
  provider: openrouter
  default: fake/model
custom_section:
  kept: true
"#,
            90,
        )
        .unwrap();

        assert_eq!(config["model"]["provider"], "openrouter");
        assert_eq!(config["model"]["default"], "fake/model");
        assert_eq!(config["custom_section"]["kept"], true);
        assert_eq!(config["display"]["skin"], "default");
    }

    #[test]
    fn legacy_root_keys_are_migrated_without_dropping_defaults() {
        let config = load_config_from_yaml(
            &defaults(),
            r#"
provider: legacy-provider
base_url: https://example.invalid/v1
max_turns: 5
"#,
            90,
        )
        .unwrap();

        assert_eq!(config["model"]["provider"], "legacy-provider");
        assert_eq!(config["model"]["base_url"], "https://example.invalid/v1");
        assert_eq!(config["agent"]["max_turns"], 5);
        assert!(config.get("provider").is_none());
        assert!(config.get("base_url").is_none());
    }

    #[test]
    fn dot_path_update_preserves_siblings() {
        let mut config = json!({
            "display": {"skin": "default", "tool_progress_command": false},
            "custom_section": {"kept": true},
        });
        set_dot_path(&mut config, "display.skin", json!("mono"));
        set_dot_path(&mut config, "new.branch.value", json!(3));

        assert_eq!(config["display"]["skin"], "mono");
        assert_eq!(config["display"]["tool_progress_command"], false);
        assert_eq!(config["custom_section"]["kept"], true);
        assert_eq!(config["new"]["branch"]["value"], 3);
    }

    #[test]
    fn discovers_and_redacts_env_values() {
        let known = ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"];
        let values = discover_env_values(
            "OPENAI_API_KEY=sk-openai\nANTHROPIC_API_KEY=sk-ant\n",
            &known,
        );
        assert_eq!(values.len(), 2);
        assert_eq!(redact_key(values["OPENAI_API_KEY"].as_str()), "sk-o...enai");
        assert_eq!(
            known_secret_metadata("ANTHROPIC_API_KEY"),
            SecretMetadata {
                category: Some("provider"),
                password: Some(true),
            }
        );
    }

    #[test]
    fn migration_dry_run_classifies_profile_without_touching_user_data() {
        let home =
            std::env::temp_dir().join(format!("hermes-migration-dry-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);

        write_fixture_file(
            &home,
            "config.yaml",
            "model:\n  provider: openrouter\ncustom:\n  kept: true\n",
        );
        write_fixture_file(&home, ".env", "OPENAI_API_KEY=sk-secret-must-not-leak\n");
        write_fixture_file(&home, "state.db", "sqlite");
        write_fixture_file(&home, "state.db-wal", "wal");
        write_fixture_file(&home, "state.db-shm", "shm");
        write_fixture_file(&home, "sessions/session.jsonl", "{}\n");
        write_fixture_file(&home, "skills/demo/SKILL.md", "---\nname: demo\n---\n");
        write_fixture_file(&home, "plugins/demo/plugin.yaml", "name: demo\n");
        write_fixture_file(&home, "cron/jobs.json", "[]\n");
        write_fixture_file(&home, "gateway/telegram/state.json", "{}\n");
        write_fixture_file(&home, "memories/MEMORY.md", "fact\n");
        write_fixture_file(&home, "memories/USER.md", "preference\n");
        write_fixture_file(&home, "tool_history/history.jsonl", "{}\n");
        write_fixture_file(&home, "checkpoints/checkpoint.json", "{}\n");
        write_fixture_file(&home, "trajectories/run.json", "{}\n");
        write_fixture_file(&home, "exports/export.json", "{}\n");
        write_fixture_file(&home, "logs/agent.log", "Authorization: Bearer sk-secret\n");
        write_fixture_file(&home, "bin/hermes", "binary");
        write_fixture_file(&home, "completions/hermes.zsh", "# completion\n");
        write_fixture_file(&home, "target/cache", "reloadable\n");

        let before_env = std::fs::read_to_string(home.join(".env")).unwrap();
        let before_config = std::fs::read_to_string(home.join("config.yaml")).unwrap();
        let report = migration_dry_run_report(&home);
        let serialized = serde_json::to_string(&report).unwrap();

        assert_eq!(report["would_write"], json!([]));
        assert_eq!(report["would_delete"], json!([]));
        assert_eq!(report["report_redacted"], true);
        assert_eq!(
            std::fs::read_to_string(home.join(".env")).unwrap(),
            before_env
        );
        assert_eq!(
            std::fs::read_to_string(home.join("config.yaml")).unwrap(),
            before_config
        );
        assert!(!serialized.contains("sk-secret-must-not-leak"));
        assert!(!serialized.contains("Authorization: Bearer"));

        let backup_required = report["backup_required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();
        for path in [
            "config.yaml",
            ".env",
            "state.db",
            "state.db-wal",
            "state.db-shm",
            "sessions",
            "skills",
            "plugins",
            "cron",
            "gateway",
            "memories/MEMORY.md",
            "memories/USER.md",
            "tool_history",
            "checkpoints",
            "trajectories",
            "exports",
            "logs",
        ] {
            assert!(backup_required.contains(&path), "{path}");
        }
        assert!(serialized.contains("\"path\":\"bin/hermes\""));
        assert!(serialized.contains("\"path\":\"completions\""));
        assert!(serialized.contains("\"path\":\"target\""));

        let _ = std::fs::remove_dir_all(home);
    }

    fn write_fixture_file(home: &Path, relative: &str, contents: &str) {
        let path = home.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
}
