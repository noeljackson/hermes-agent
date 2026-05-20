use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::process::Command;

const DEFAULT_IMAGE: &str = "nikolaik/python-nodejs:python3.11-nodejs20";
const FOREGROUND_MAX_TIMEOUT: i64 = 600;

pub fn resolve_env_config(env: &BTreeMap<String, String>, current_dir: &str) -> Value {
    resolve_env_config_result(env, current_dir).expect("valid terminal environment config")
}

pub fn terminal_tool_value(command: &Value, timeout: Option<i64>) -> Value {
    let Some(command) = command.as_str() else {
        return json!({
            "error": format!("Invalid command: expected string, got {}", python_type_name(command)),
            "exit_code": -1,
            "output": "",
            "status": "error",
        });
    };

    if let Some(timeout) = timeout {
        if timeout > FOREGROUND_MAX_TIMEOUT {
            return json!({
                "error": format!(
                    "Foreground timeout {timeout}s exceeds the maximum of {FOREGROUND_MAX_TIMEOUT}s. Use background=true with notify_on_complete=true for long-running commands."
                ),
            });
        }
    }

    execute_local_command(command)
}

pub fn execute_local_command(command: &str) -> Value {
    match Command::new("sh").arg("-c").arg(command).output() {
        Ok(output) => {
            let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
            if combined.is_empty() && !output.stderr.is_empty() {
                combined = String::from_utf8_lossy(&output.stderr).to_string();
            }
            json!({
                "error": Value::Null,
                "exit_code": output.status.code().unwrap_or(-1),
                "output": combined.trim().to_string(),
            })
        }
        Err(error) => json!({
            "error": format!("Command execution failed: {error}"),
            "exit_code": -1,
            "output": "",
        }),
    }
}

pub fn resolve_env_config_result(
    env: &BTreeMap<String, String>,
    current_dir: &str,
) -> Result<Value, String> {
    let env_type = env
        .get("TERMINAL_ENV")
        .map(String::as_str)
        .unwrap_or("local");
    let mount_docker_cwd = bool_env(env, "TERMINAL_DOCKER_MOUNT_CWD_TO_WORKSPACE", false);
    let default_cwd = match env_type {
        "local" => current_dir.to_string(),
        "ssh" => "/root".to_string(),
        "vercel_sandbox" => "/vercel/sandbox".to_string(),
        _ => "/root".to_string(),
    };

    let mut cwd = env
        .get("TERMINAL_CWD")
        .cloned()
        .unwrap_or(default_cwd.clone());
    let mut host_cwd = Value::Null;

    if env_type == "docker" && mount_docker_cwd {
        let candidate = env
            .get("TERMINAL_CWD")
            .cloned()
            .unwrap_or_else(|| current_dir.to_string());
        if is_host_path(&candidate) {
            host_cwd = json!(candidate);
            cwd = "/workspace".to_string();
        }
    } else if matches!(
        env_type,
        "modal" | "docker" | "singularity" | "daytona" | "vercel_sandbox"
    ) && (is_host_path(&cwd) || !cwd.starts_with('/'))
        && cwd != default_cwd
    {
        cwd = default_cwd;
    }

    Ok(json!({
        "container_cpu": number_env(env, "TERMINAL_CONTAINER_CPU", 1.0)?,
        "container_disk": int_env(env, "TERMINAL_CONTAINER_DISK", 51200)?,
        "container_memory": int_env(env, "TERMINAL_CONTAINER_MEMORY", 5120)?,
        "container_persistent": bool_env(env, "TERMINAL_CONTAINER_PERSISTENT", true),
        "cwd": cwd,
        "docker_env": json_env(env, "TERMINAL_DOCKER_ENV", json!({}))?,
        "docker_extra_args": json_env(env, "TERMINAL_DOCKER_EXTRA_ARGS", json!([]))?,
        "docker_forward_env": json_env(env, "TERMINAL_DOCKER_FORWARD_ENV", json!([]))?,
        "docker_image": env.get("TERMINAL_DOCKER_IMAGE").map(String::as_str).unwrap_or(DEFAULT_IMAGE),
        "docker_mount_cwd_to_workspace": mount_docker_cwd,
        "docker_volumes": json_env(env, "TERMINAL_DOCKER_VOLUMES", json!([]))?,
        "env_type": env_type,
        "host_cwd": host_cwd,
        "local_persistent": bool_env(env, "TERMINAL_LOCAL_PERSISTENT", false),
        "modal_mode": coerce_modal_mode(env.get("TERMINAL_MODAL_MODE").map(String::as_str).unwrap_or("auto")),
        "ssh_host": env.get("TERMINAL_SSH_HOST").map(String::as_str).unwrap_or(""),
        "ssh_key_present": env.get("TERMINAL_SSH_KEY").is_some_and(|value| !value.is_empty()),
        "ssh_persistent": bool_env(env, "TERMINAL_SSH_PERSISTENT", bool_env(env, "TERMINAL_PERSISTENT_SHELL", true)),
        "ssh_port": int_env(env, "TERMINAL_SSH_PORT", 22)?,
        "ssh_user": env.get("TERMINAL_SSH_USER").map(String::as_str).unwrap_or(""),
        "timeout": int_env(env, "TERMINAL_TIMEOUT", 180)?,
    }))
}

fn is_host_path(path: &str) -> bool {
    path.starts_with("/Users/")
        || path.starts_with("/home/")
        || path.starts_with("C:\\")
        || path.starts_with("C:/")
}

fn bool_env(env: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    env.get(key)
        .map(|value| matches!(value.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(default)
}

fn int_env(env: &BTreeMap<String, String>, key: &str, default: i64) -> Result<i64, String> {
    env.get(key)
        .map(|value| {
            value
                .parse()
                .map_err(|_| parse_error(key, value, "integer"))
        })
        .unwrap_or(Ok(default))
}

fn number_env(env: &BTreeMap<String, String>, key: &str, default: f64) -> Result<f64, String> {
    env.get(key)
        .map(|value| value.parse().map_err(|_| parse_error(key, value, "number")))
        .unwrap_or(Ok(default))
}

fn json_env(env: &BTreeMap<String, String>, key: &str, default: Value) -> Result<Value, String> {
    env.get(key)
        .map(|value| serde_json::from_str(value).map_err(|_| parse_error(key, value, "valid JSON")))
        .unwrap_or(Ok(default))
}

fn parse_error(key: &str, value: &str, expected: &str) -> String {
    format!(
        "Invalid value for {key}: '{value}' (expected {expected}). Check ~/.hermes/.env or environment variables."
    )
}

fn coerce_modal_mode(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "auto" => "auto".to_string(),
        "direct" => "direct".to_string(),
        "managed" => "managed".to_string(),
        _ => "auto".to_string(),
    }
}

fn python_type_name(value: &Value) -> &'static str {
    match value {
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
        Value::String(_) => "str",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::Null => "NoneType",
    }
}
