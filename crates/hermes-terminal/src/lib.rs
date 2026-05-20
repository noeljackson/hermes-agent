use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
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

pub fn normalize_forward_env_names(value: &Value) -> Value {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for item in value.as_array().into_iter().flatten() {
        let Some(raw) = item.as_str() else {
            continue;
        };
        let key = raw.trim();
        if key.is_empty() || !is_valid_env_name(key) || !seen.insert(key.to_string()) {
            continue;
        }
        out.push(json!(key));
    }
    Value::Array(out)
}

pub fn normalize_docker_env_dict(value: &Value) -> Value {
    let mut out = serde_json::Map::new();
    let Some(input) = value.as_object() else {
        return Value::Object(out);
    };
    for (key, value) in input {
        let key = key.trim();
        if !is_valid_env_name(key) {
            continue;
        }
        let normalized = match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(if *flag { "True" } else { "False" }.to_string()),
            _ => None,
        };
        if let Some(normalized) = normalized {
            out.insert(key.to_string(), json!(normalized));
        }
    }
    Value::Object(out)
}

pub fn docker_security_args(run_as_host_user: bool) -> Vec<String> {
    let mut args = vec![
        "--cap-drop",
        "ALL",
        "--cap-add",
        "DAC_OVERRIDE",
        "--cap-add",
        "CHOWN",
        "--cap-add",
        "FOWNER",
        "--security-opt",
        "no-new-privileges",
        "--pids-limit",
        "256",
        "--tmpfs",
        "/tmp:rw,nosuid,size=512m",
        "--tmpfs",
        "/var/tmp:rw,noexec,nosuid,size=256m",
        "--tmpfs",
        "/run:rw,noexec,nosuid,size=64m",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    if !run_as_host_user {
        args.extend(
            ["--cap-add", "SETUID", "--cap-add", "SETGID"]
                .into_iter()
                .map(str::to_string),
        );
    }
    args
}

pub fn ssh_command(
    control_path: &str,
    host: &str,
    user: &str,
    port: i64,
    key_path: &str,
    extra_args: &[&str],
) -> Vec<String> {
    let mut cmd = vec![
        "ssh".to_string(),
        "-o".to_string(),
        format!("ControlPath={control_path}"),
        "-o".to_string(),
        "ControlMaster=auto".to_string(),
        "-o".to_string(),
        "ControlPersist=300".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
    ];
    if port != 22 {
        cmd.extend(["-p".to_string(), port.to_string()]);
    }
    if !key_path.is_empty() {
        cmd.extend(["-i".to_string(), key_path.to_string()]);
    }
    cmd.extend(extra_args.iter().map(|arg| (*arg).to_string()));
    cmd.push(format!("{user}@{host}"));
    cmd
}

pub fn quoted_mkdir_command(dirs: &[&str]) -> String {
    format!(
        "mkdir -p {}",
        dirs.iter()
            .map(|dir| shell_quote(dir))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub fn quoted_rm_command(paths: &[&str]) -> String {
    format!(
        "rm -f {}",
        paths
            .iter()
            .map(|path| shell_quote(path))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub fn unique_parent_dirs(files: &[(&str, &str)]) -> Vec<String> {
    files
        .iter()
        .filter_map(|(_, remote)| {
            remote
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_string())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn modal_direct_snapshot_key(task_id: &str) -> String {
    format!("direct:{task_id}")
}

pub fn modal_restore_candidate(snapshots: &Value, task_id: &str) -> Value {
    let Some(map) = snapshots.as_object() else {
        return json!([null, false]);
    };
    let direct_key = modal_direct_snapshot_key(task_id);
    if let Some(value) = map.get(&direct_key).and_then(Value::as_str) {
        if !value.is_empty() {
            return json!([value, false]);
        }
    }
    if let Some(value) = map.get(task_id).and_then(Value::as_str) {
        if !value.is_empty() {
            return json!([value, true]);
        }
    }
    json!([null, false])
}

pub fn modal_delete_direct_snapshot(
    snapshots: &Value,
    task_id: &str,
    snapshot_id: Option<&str>,
) -> Value {
    let mut map = snapshots.as_object().cloned().unwrap_or_default();
    for key in [modal_direct_snapshot_key(task_id), task_id.to_string()] {
        let should_remove = map.get(&key).is_some_and(|value| {
            snapshot_id.is_none_or(|expected| value.as_str() == Some(expected))
        });
        if should_remove {
            map.remove(&key);
        }
    }
    Value::Object(map)
}

pub fn modal_store_direct_snapshot(snapshots: &Value, task_id: &str, snapshot_id: &str) -> Value {
    let mut map = snapshots.as_object().cloned().unwrap_or_default();
    map.insert(modal_direct_snapshot_key(task_id), json!(snapshot_id));
    map.remove(task_id);
    Value::Object(map)
}

pub fn base_modal_contracts_fixture(case: &Value) -> Value {
    json!({
        "cwd_marker": cwd_marker("abc123"),
        "embedded_stdin": embed_stdin_heredoc("cat", "hello\nworld", "deadbeef"),
        "modal_stdin": wrap_modal_stdin_heredoc(
            "cat",
            "first HERMES_EOF_deadbeef marker",
            &["deadbeef", "cafebabe"],
        ),
        "modal_sudo": wrap_modal_sudo_pipe("sudo -S true", "pa ss\n"),
        "quote_cwd": case["quote_cwd"].as_array().unwrap().iter().map(|item| {
            let cwd = item["cwd"].as_str().unwrap_or("");
            json!({"cwd": cwd, "quoted": quote_cwd_for_cd(cwd)})
        }).collect::<Vec<_>>(),
        "wrapped_snapshot_ready": wrap_environment_command(
            "printf 'hi'",
            "~/work dir",
            "/tmp/snap path.sh",
            "/tmp/cwd path.txt",
            "abc123def456",
            true,
        ),
    })
}

fn cwd_marker(session_id: &str) -> String {
    format!("__HERMES_CWD_{session_id}__")
}

fn quote_cwd_for_cd(cwd: &str) -> String {
    if cwd == "~" {
        return cwd.to_string();
    }
    if cwd == "~/" {
        return "$HOME".to_string();
    }
    if let Some(suffix) = cwd.strip_prefix("~/") {
        return format!("$HOME/{}", shell_quote(suffix));
    }
    shell_quote(cwd)
}

fn wrap_environment_command(
    command: &str,
    cwd: &str,
    snapshot_path: &str,
    cwd_file: &str,
    session_id: &str,
    snapshot_ready: bool,
) -> String {
    let marker = cwd_marker(session_id);
    let escaped = command.replace('\'', "'\\''");
    let quoted_snap = shell_quote(snapshot_path);
    let quoted_cwd_file = shell_quote(cwd_file);
    let mut parts = Vec::new();
    if snapshot_ready {
        parts.push(format!("source {quoted_snap} >/dev/null 2>&1 || true"));
    }
    parts.push(format!(
        "builtin cd -- {} || exit 126",
        quote_cwd_for_cd(cwd)
    ));
    parts.push(format!("eval '{escaped}'"));
    parts.push("__hermes_ec=$?".to_string());
    if snapshot_ready {
        parts.push(format!("export -p > {quoted_snap} 2>/dev/null || true"));
    }
    parts.push(format!("pwd -P > {quoted_cwd_file} 2>/dev/null || true"));
    parts.push(format!("printf '\\n{marker}%s{marker}\\n' \"$(pwd -P)\""));
    parts.push("exit $__hermes_ec".to_string());
    parts.join("\n")
}

fn embed_stdin_heredoc(command: &str, stdin_data: &str, uuid_hex: &str) -> String {
    let delimiter = format!("HERMES_STDIN_{}", &uuid_hex[..12.min(uuid_hex.len())]);
    format!("{command} << '{delimiter}'\n{stdin_data}\n{delimiter}")
}

fn wrap_modal_stdin_heredoc(command: &str, stdin_data: &str, uuid_hexes: &[&str]) -> String {
    let mut marker = String::new();
    for uuid_hex in uuid_hexes {
        marker = format!("HERMES_EOF_{}", &uuid_hex[..8.min(uuid_hex.len())]);
        if !stdin_data.contains(&marker) {
            break;
        }
    }
    format!("{command} << '{marker}'\n{stdin_data}\n{marker}")
}

fn wrap_modal_sudo_pipe(command: &str, sudo_stdin: &str) -> String {
    format!(
        "printf '%s\\n' {} | {command}",
        shell_quote(sudo_stdin.trim_end())
    )
}

fn is_valid_env_name(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
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
