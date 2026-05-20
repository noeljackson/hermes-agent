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

pub fn terminal_safety_helpers_fixture(case: &Value) -> Value {
    json!({
        "name": "terminal_safety_helpers",
        "compound_background": case["compound_background"].as_array().unwrap().iter().map(|item| {
            let command = item["command"].as_str().unwrap_or("");
            json!({"command": command, "rewritten": rewrite_compound_background(command)})
        }).collect::<Vec<_>>(),
        "foreground_guidance": case["foreground_guidance"].as_array().unwrap().iter().map(|item| {
            let command = item["command"].as_str().unwrap_or("");
            json!({"command": command, "guidance": foreground_background_guidance(command)})
        }).collect::<Vec<_>>(),
        "notification_conflicts": case["notification_conflicts"].as_array().unwrap().iter().map(|item| {
            let background = item["background"].as_bool().unwrap_or(false);
            let notify_on_complete = item["notify_on_complete"].as_bool().unwrap_or(false);
            let watch_patterns = item["watch_patterns"].clone();
            let (resolved_watch_patterns, note) =
                resolve_notification_flag_conflict(background, notify_on_complete, &watch_patterns);
            json!({
                "background": background,
                "notify_on_complete": notify_on_complete,
                "watch_patterns": watch_patterns,
                "resolved_watch_patterns": resolved_watch_patterns,
                "note": note,
            })
        }).collect::<Vec<_>>(),
        "sudo_transform": case["sudo_transform"].as_array().unwrap().iter().map(|item| {
            let command = item["command"].as_str().unwrap_or("");
            let password = item["password_present"].as_bool().unwrap_or(false).then_some("pa ss");
            let (transformed, sudo_stdin) = transform_sudo_command(command, password);
            json!({
                "command": command,
                "password_present": item["password_present"],
                "transformed": transformed,
                "sudo_stdin": sudo_stdin,
            })
        }).collect::<Vec<_>>(),
    })
}

fn cwd_marker(session_id: &str) -> String {
    format!("__HERMES_CWD_{session_id}__")
}

fn rewrite_compound_background(command: &str) -> String {
    let bytes = command.as_bytes();
    let mut i = 0;
    let mut paren_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut last_chain_op_end: Option<usize> = None;
    let mut rewrites = Vec::<(usize, usize)>::new();

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '\n' && paren_depth == 0 && brace_depth == 0 {
            last_chain_op_end = None;
            i += 1;
            continue;
        }
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if ch == '#' {
            if let Some(offset) = command[i..].find('\n') {
                i += offset;
            } else {
                break;
            }
            continue;
        }
        if ch == '\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if ch == '\'' || ch == '"' {
            i = read_shell_token_end(command, i).max(i + 1);
            continue;
        }
        if ch == '(' {
            paren_depth += 1;
            i += 1;
            continue;
        }
        if ch == ')' {
            paren_depth = paren_depth.saturating_sub(1);
            i += 1;
            continue;
        }
        if ch == '{'
            && i + 1 < bytes.len()
            && ((bytes[i + 1] as char).is_ascii_whitespace() || bytes[i + 1] == b'\n')
        {
            brace_depth += 1;
            i += 1;
            continue;
        }
        if ch == '}' && brace_depth > 0 {
            brace_depth -= 1;
            last_chain_op_end = None;
            i += 1;
            continue;
        }
        if paren_depth > 0 || brace_depth > 0 {
            i += 1;
            continue;
        }
        if command[i..].starts_with("&&") || command[i..].starts_with("||") {
            last_chain_op_end = Some(i + 2);
            i += 2;
            continue;
        }
        if ch == ';' || ch == '|' {
            last_chain_op_end = None;
            i += 1;
            continue;
        }
        if ch == '&' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                i += 2;
                continue;
            }
            let mut j = i;
            while j > 0 && (bytes[j - 1] as char).is_ascii_whitespace() {
                j -= 1;
            }
            if j > 0 && matches!(bytes[j - 1], b'<' | b'>') {
                i += 1;
                continue;
            }
            if let Some(chain_end) = last_chain_op_end {
                rewrites.push((chain_end, i));
            }
            last_chain_op_end = None;
            i += 1;
            continue;
        }
        i = read_shell_token_end(command, i).max(i + 1);
    }

    if rewrites.is_empty() {
        return command.to_string();
    }

    let mut result = command.to_string();
    for (chain_end, amp_pos) in rewrites.into_iter().rev() {
        let mut insert_pos = chain_end;
        while insert_pos < amp_pos && result.as_bytes()[insert_pos].is_ascii_whitespace() {
            insert_pos += 1;
        }
        let prefix = &result[..insert_pos];
        let middle = &result[insert_pos..amp_pos];
        let suffix = &result[amp_pos + 1..];
        result = format!("{prefix}{{ {middle}& }}{suffix}");
    }
    result
}

fn foreground_background_guidance(command: &str) -> Option<String> {
    if looks_like_help_or_version_command(command) {
        return None;
    }
    let unquoted = strip_quotes(command).to_ascii_lowercase();
    if contains_shell_level_background_wrapper(&unquoted) {
        return Some(
            "Foreground command uses shell-level background wrappers (nohup/disown/setsid). Use terminal(background=true) so Hermes can track the process, then run readiness checks and tests in separate commands."
                .to_string(),
        );
    }
    if contains_inline_background_amp(&unquoted) || contains_trailing_background_amp(&unquoted) {
        return Some(
            "Foreground command uses '&' backgrounding. Use terminal(background=true) for long-lived processes, then run health checks and tests in follow-up terminal calls."
                .to_string(),
        );
    }
    if looks_long_lived(&unquoted) {
        return Some(
            "This foreground command appears to start a long-lived server/watch process. Run it with background=true, verify readiness (health endpoint/log signal), then execute tests in a separate command."
                .to_string(),
        );
    }
    None
}

fn resolve_notification_flag_conflict(
    background: bool,
    notify_on_complete: bool,
    watch_patterns: &Value,
) -> (Value, String) {
    if background
        && notify_on_complete
        && watch_patterns
            .as_array()
            .is_some_and(|patterns| !patterns.is_empty())
    {
        return (
            Value::Null,
            "watch_patterns ignored because notify_on_complete=True; these two flags produce duplicate notifications when combined"
                .to_string(),
        );
    }
    (watch_patterns.clone(), String::new())
}

fn transform_sudo_command(command: &str, password: Option<&str>) -> (String, Option<String>) {
    let (transformed, found) = rewrite_real_sudo_invocations(command);
    if found {
        if let Some(password) = password {
            return (transformed, Some(format!("{password}\n")));
        }
    }
    (command.to_string(), None)
}

fn rewrite_real_sudo_invocations(command: &str) -> (String, bool) {
    let mut out = String::new();
    let bytes = command.as_bytes();
    let mut i = 0;
    let mut command_start = true;
    let mut found = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_whitespace() {
            out.push(ch);
            if ch == '\n' {
                command_start = true;
            }
            i += 1;
            continue;
        }
        if ch == '#' && command_start {
            if let Some(offset) = command[i..].find('\n') {
                out.push_str(&command[i..i + offset]);
                i += offset;
            } else {
                out.push_str(&command[i..]);
                break;
            }
            continue;
        }
        if command[i..].starts_with("&&")
            || command[i..].starts_with("||")
            || command[i..].starts_with(";;")
        {
            out.push_str(&command[i..i + 2]);
            i += 2;
            command_start = true;
            continue;
        }
        if matches!(ch, ';' | '|' | '&' | '(') {
            out.push(ch);
            i += 1;
            command_start = true;
            continue;
        }
        if ch == ')' {
            out.push(ch);
            i += 1;
            command_start = false;
            continue;
        }

        let next_i = read_shell_token_end(command, i);
        let token = &command[i..next_i];
        if command_start && token == "sudo" {
            out.push_str("sudo -S -p ''");
            found = true;
        } else {
            out.push_str(token);
        }
        command_start = command_start && looks_like_env_assignment(token);
        i = next_i.max(i + 1);
    }
    (out, found)
}

fn read_shell_token_end(command: &str, start: usize) -> usize {
    let bytes = command.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_whitespace() || matches!(ch, ';' | '|' | '&' | '(' | ')') {
            break;
        }
        if ch == '\'' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\'' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        if ch == '"' {
            i += 1;
            while i < bytes.len() {
                let inner = bytes[i] as char;
                if inner == '\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if inner == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if ch == '\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        i += 1;
    }
    i
}

fn looks_like_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn looks_like_help_or_version_command(command: &str) -> bool {
    let normalized = command
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    normalized.contains(" --help")
        || normalized.ends_with(" -h")
        || normalized.contains(" --version")
        || normalized.ends_with(" -v")
}

fn strip_quotes(command: &str) -> String {
    let mut out = String::with_capacity(command.len());
    let bytes = command.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '\'' {
            out.push_str("''");
            i += 1;
            while i < bytes.len() && bytes[i] != b'\'' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
        } else if ch == '"' {
            out.push_str("\"\"");
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else if ch == '`' {
            out.push_str("``");
            i += 1;
            while i < bytes.len() && bytes[i] != b'`' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

fn contains_shell_level_background_wrapper(command: &str) -> bool {
    for needle in ["nohup", "disown", "setsid"] {
        if command
            .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ';' | '&' | '|' | '(' | '$'))
            .any(|token| token == needle)
        {
            return true;
        }
    }
    false
}

fn contains_inline_background_amp(command: &str) -> bool {
    command.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_whitespace() && window[1] == b'&' && window[2].is_ascii_whitespace()
    })
}

fn contains_trailing_background_amp(command: &str) -> bool {
    let stripped = command
        .split_once('#')
        .map(|(prefix, _)| prefix)
        .unwrap_or(command)
        .trim_end();
    stripped.ends_with(" &")
}

fn looks_long_lived(command: &str) -> bool {
    command.contains("docker compose up")
        || command.contains("next dev")
        || command.contains("nodemon")
        || command.contains("uvicorn")
        || command.contains("gunicorn")
        || command.contains("python -m http.server")
        || command.contains("python3 -m http.server")
        || command.split_whitespace().any(|token| token == "vite")
        || command.contains("npm run dev")
        || command.contains("npm run start")
        || command.contains("npm run serve")
        || command.contains("npm run watch")
        || command.contains("pnpm run dev")
        || command.contains("yarn run dev")
        || command.contains("bun run dev")
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
