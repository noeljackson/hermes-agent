use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolSummary {
    pub name: &'static str,
    pub toolset: &'static str,
    pub is_async: bool,
    pub requires_env: &'static [&'static str],
    pub parameter_names: &'static [&'static str],
    pub required: &'static [&'static str],
    pub description_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolsetInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub direct_tools: &'static [&'static str],
    pub includes: &'static [&'static str],
    pub resolved_tools: &'static [&'static str],
    pub is_composite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolParamContract {
    pub tool: &'static str,
    pub parameter: &'static str,
    pub json_type: &'static str,
    pub default_json: Option<&'static str>,
    pub enum_values: &'static [&'static str],
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyReplaceResult {
    pub content: String,
    pub count: usize,
    pub strategy: Option<&'static str>,
    pub error: Option<String>,
}

pub fn tool_error(message: &str, extra: &[(&str, Value)]) -> Value {
    let mut result = serde_json::Map::new();
    result.insert("error".to_string(), json!(message));
    for (key, value) in extra {
        result.insert((*key).to_string(), value.clone());
    }
    Value::Object(result)
}

pub fn clarify_tool(question: &str, choices: Option<&[&str]>, callback_available: bool) -> Value {
    let question = question.trim();
    if question.is_empty() {
        return tool_error("Question text is required.", &[]);
    }

    let choices = choices.and_then(|choices| {
        let cleaned = choices
            .iter()
            .filter_map(|choice| {
                let choice = choice.trim();
                (!choice.is_empty()).then_some(choice.to_string())
            })
            .take(4)
            .collect::<Vec<_>>();
        (!cleaned.is_empty()).then_some(cleaned)
    });

    if !callback_available {
        return json!({"error": "Clarify tool is not available in this execution context."});
    }

    json!({
        "choices_offered": choices,
        "question": question,
        "user_response": "",
    })
}

pub fn handle_function_call_selected(function_name: &str, _args: &Value) -> Value {
    match function_name {
        "todo" | "memory" | "session_search" | "delegate_task" => {
            json!({"error": format!("{function_name} must be handled by the agent loop")})
        }
        "clarify" => clarify_tool("", None, false),
        _ => json!({"error": format!("Unknown tool: {function_name}")}),
    }
}

pub fn normalize_read_pagination(offset: impl ToString, limit: impl ToString) -> (i64, i64) {
    let offset = offset.to_string().parse::<i64>().unwrap_or(1).max(1);
    let limit = limit
        .to_string()
        .parse::<i64>()
        .unwrap_or(500)
        .clamp(1, 2000);
    (offset, limit)
}

pub fn normalize_search_pagination(offset: impl ToString, limit: impl ToString) -> (i64, i64) {
    let offset = offset.to_string().parse::<i64>().unwrap_or(0).max(0);
    let limit = limit.to_string().parse::<i64>().unwrap_or(50).max(1);
    (offset, limit)
}

pub fn is_write_denied(path: &str) -> bool {
    let path = path.strip_prefix("~/").unwrap_or(path);
    matches!(path, ".ssh/authorized_keys" | ".ssh/id_rsa" | ".netrc")
        || path.starts_with(".aws/")
        || path.starts_with(".kube/")
}

pub fn is_likely_binary(path: &str, sample: Option<&str>) -> bool {
    let ext = path
        .rsplit_once('.')
        .map(|(_, ext)| format!(".{}", ext.to_ascii_lowercase()))
        .unwrap_or_default();
    if BINARY_EXTENSIONS.contains(&ext.as_str()) {
        return true;
    }
    let Some(sample) = sample else {
        return false;
    };
    if sample.is_empty() {
        return false;
    }
    let len = sample.chars().take(1000).count().min(1000);
    let non_printable = sample
        .chars()
        .take(1000)
        .filter(|ch| (*ch as u32) < 32 && !matches!(ch, '\n' | '\r' | '\t'))
        .count();
    (non_printable as f64 / len as f64) > 0.30
}

pub fn is_image(path: &str) -> bool {
    let ext = path
        .rsplit_once('.')
        .map(|(_, ext)| format!(".{}", ext.to_ascii_lowercase()))
        .unwrap_or_default();
    IMAGE_EXTENSIONS.contains(&ext.as_str())
}

pub fn add_line_numbers(content: &str, start_line: usize) -> String {
    content
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            let mut line = line.to_string();
            if line.chars().count() > 2000 {
                line = line.chars().take(2000).collect::<String>() + "... [truncated]";
            }
            format!("{:6}|{}", start_line + index, line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn fuzzy_find_and_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> FuzzyReplaceResult {
    if old_string.is_empty() {
        return fuzzy_error(content, "old_string cannot be empty");
    }
    if old_string == new_string {
        return fuzzy_error(content, "old_string and new_string are identical");
    }

    let matches = find_all(content, old_string);
    if !matches.is_empty() {
        return apply_fuzzy_matches(content, new_string, matches, replace_all, "exact");
    }

    let normalized_content = unicode_normalize(content);
    let normalized_old = unicode_normalize(old_string);
    let normalized_matches = find_all(&normalized_content, &normalized_old);
    if !normalized_matches.is_empty() && normalized_content.len() == content.len() {
        return apply_fuzzy_matches(
            content,
            new_string,
            normalized_matches,
            replace_all,
            "unicode_normalized",
        );
    }

    fuzzy_error(content, "Could not find a match for old_string in the file")
}

fn fuzzy_error(content: &str, error: &str) -> FuzzyReplaceResult {
    FuzzyReplaceResult {
        content: content.to_string(),
        count: 0,
        strategy: None,
        error: Some(error.to_string()),
    }
}

fn find_all(content: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut matches = Vec::new();
    let mut start = 0;
    while let Some(offset) = content[start..].find(needle) {
        let begin = start + offset;
        let end = begin + needle.len();
        matches.push((begin, end));
        start = end;
    }
    matches
}

fn apply_fuzzy_matches(
    content: &str,
    new_string: &str,
    matches: Vec<(usize, usize)>,
    replace_all: bool,
    strategy: &'static str,
) -> FuzzyReplaceResult {
    if matches.len() > 1 && !replace_all {
        return fuzzy_error(
            content,
            &format!(
                "Found {} matches for old_string. Provide more context to make it unique, or use replace_all=True.",
                matches.len()
            ),
        );
    }

    let mut out = String::new();
    let mut cursor = 0;
    for (begin, end) in &matches {
        out.push_str(&content[cursor..*begin]);
        out.push_str(new_string);
        cursor = *end;
        if !replace_all {
            break;
        }
    }
    out.push_str(&content[cursor..]);

    FuzzyReplaceResult {
        content: out,
        count: if replace_all { matches.len() } else { 1 },
        strategy: Some(strategy),
        error: None,
    }
}

fn unicode_normalize(text: &str) -> String {
    text.replace(['\u{201c}', '\u{201d}'], "\"")
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace('\u{2014}', "--")
        .replace('\u{2013}', "-")
        .replace('\u{2026}', "...")
        .replace('\u{00a0}', " ")
}

pub fn read_file_handler(args: &Value, cwd: &Path) -> Value {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("");
    let (offset, limit) = normalize_read_pagination(
        args.get("offset").unwrap_or(&json!(1)),
        args.get("limit").unwrap_or(&json!(500)),
    );
    let path = resolve_local_path(cwd, path);
    match read_text_file(&path) {
        Ok(content) => {
            let file_size = content.len();
            let total_lines = content.lines().count();
            let end_line = offset + limit - 1;
            let lines = content.split('\n').collect::<Vec<_>>();
            let selected = lines
                .iter()
                .enumerate()
                .filter_map(|(index, line)| {
                    let line_no = index as i64 + 1;
                    (line_no >= offset && line_no <= end_line).then_some(*line)
                })
                .collect::<Vec<_>>();
            let mut read_output = selected.join("\n");
            if !selected.is_empty() && content.ends_with('\n') {
                read_output.push('\n');
            }
            let mut result = json!({
                "content": add_line_numbers(&read_output, offset as usize),
                "total_lines": total_lines,
                "file_size": file_size,
                "truncated": (total_lines as i64) > end_line,
                "is_binary": false,
                "is_image": false,
            });
            if result["truncated"] == json!(true) {
                result["_hint"] = json!(format!(
                    "Use offset={} to continue reading (showing {}-{} of {} lines)",
                    end_line + 1,
                    offset,
                    end_line,
                    total_lines
                ));
            }
            result
        }
        Err(_) => tool_error(
            &format!(
                "File not found: {}",
                args.get("path").and_then(Value::as_str).unwrap_or("")
            ),
            &[],
        ),
    }
}

pub fn write_file_handler(args: &Value, cwd: &Path) -> Value {
    let Some(path) = args.get("path").and_then(Value::as_str) else {
        return tool_error(
            "write_file: missing required field 'path'. Re-emit the tool call with both 'path' and 'content' set.",
            &[],
        );
    };
    let Some(content) = args.get("content") else {
        return tool_error(
            "write_file: missing required field 'content'. The tool call included a path but no content argument — this is almost always a dropped-arg bug under context pressure. Re-emit the tool call with the full content payload, or use execute_code with hermes_tools.write_file() for very large files.",
            &[],
        );
    };
    let Some(content) = content.as_str() else {
        return tool_error(
            &format!(
                "write_file: 'content' must be a string, got {}.",
                json_type_name(content)
            ),
            &[],
        );
    };

    let path = resolve_local_path(cwd, path);
    let dirs_created = ensure_parent_dir(&path).unwrap_or(false);
    if let Err(error) = fs::write(&path, content) {
        return tool_error(&format!("Failed to write file: {error}"), &[]);
    }
    json!({
        "bytes_written": content.len(),
        "dirs_created": dirs_created,
        "lint": lint_skipped_for_path(&path),
    })
}

pub fn patch_handler(args: &Value, cwd: &Path) -> Value {
    let mode = args
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("replace");
    if mode != "replace" {
        return tool_error(&format!("Unknown mode: {mode}"), &[]);
    }
    let Some(path_arg) = args.get("path").and_then(Value::as_str) else {
        return tool_error("path required", &[]);
    };
    let Some(old_string) = args.get("old_string").and_then(Value::as_str) else {
        return tool_error("old_string and new_string required", &[]);
    };
    let Some(new_string) = args.get("new_string").and_then(Value::as_str) else {
        return tool_error("old_string and new_string required", &[]);
    };
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let path = resolve_local_path(cwd, path_arg);
    let Ok(content) = read_text_file(&path) else {
        return tool_error(&format!("Failed to read file: {path_arg}"), &[]);
    };
    let replaced = fuzzy_find_and_replace(&content, old_string, new_string, replace_all);
    if let Some(error) = replaced.error {
        return tool_error(&error, &[]);
    }
    if let Err(error) = fs::write(&path, &replaced.content) {
        return tool_error(&format!("Failed to write changes: {error}"), &[]);
    }
    json!({
        "success": true,
        "diff": unified_diff(&content, &replaced.content, path_arg),
        "files_modified": [path_arg],
        "lint": lint_skipped_for_path(&path),
    })
}

pub fn search_files_handler(args: &Value, cwd: &Path) -> Value {
    let pattern = args.get("pattern").and_then(Value::as_str).unwrap_or("");
    let target = args
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("content");
    let path = resolve_local_path(cwd, args.get("path").and_then(Value::as_str).unwrap_or("."));
    let (_offset, limit) = normalize_search_pagination(
        args.get("offset").unwrap_or(&json!(0)),
        args.get("limit").unwrap_or(&json!(50)),
    );

    if target == "files" {
        let mut files = Vec::new();
        collect_matching_files(&path, cwd, pattern, &mut files);
        files.sort();
        files.truncate(limit as usize);
        return json!({
            "files": files,
            "total_count": files.len(),
        });
    }

    json!({"total_count": 0})
}

pub fn memory_tool_handler(args: &Value, memory_dir: &Path) -> Value {
    let mut store = match hermes_memory::MemoryStore::load_from_dir(memory_dir, 500, 500) {
        Ok(store) => store,
        Err(error) => {
            return tool_error(
                &format!(
                    "Memory is not available. It may be disabled in config or this environment: {error}"
                ),
                &[],
            );
        }
    };
    let action = args.get("action").and_then(Value::as_str).unwrap_or("");
    let target = args
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("memory");
    let content = args.get("content").and_then(Value::as_str);
    let old_text = args.get("old_text").and_then(Value::as_str);
    let result = hermes_memory::memory_tool(&mut store, action, target, content, old_text);
    if result
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && matches!(target, "memory" | "user")
    {
        if let Err(error) = store.save_to_dir(memory_dir, target) {
            return tool_error(&format!("Failed to save memory: {error}"), &[]);
        }
    }
    result
}

pub fn skills_list_handler(args: &Value, skills_root: &Path) -> Value {
    let category = args.get("category").and_then(Value::as_str);
    match hermes_skills::skills_list_json(skills_root, category) {
        Ok(value) => value,
        Err(error) => tool_error(&error.to_string(), &[]),
    }
}

fn resolve_local_path(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn read_text_file(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

fn ensure_parent_dir(path: &Path) -> io::Result<bool> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    if parent.as_os_str().is_empty() || parent.exists() {
        return Ok(false);
    }
    fs::create_dir_all(parent)?;
    Ok(true)
}

fn lint_skipped_for_path(path: &Path) -> Value {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    json!({
        "status": "skipped",
        "message": format!("No linter for {extension} files"),
    })
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn unified_diff(old: &str, new: &str, filename: &str) -> String {
    let old_lines = split_keepends(old);
    let new_lines = split_keepends(new);
    let first_diff = old_lines
        .iter()
        .zip(new_lines.iter())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| old_lines.len().min(new_lines.len()));
    let old_start = first_diff.saturating_sub(3);
    let old_end = old_lines.len().min(first_diff + 4);
    let new_end = new_lines.len().min(first_diff + 4);
    let mut diff = format!("--- a/{filename}\n+++ b/{filename}\n");
    diff.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        old_start + 1,
        old_end.saturating_sub(old_start),
        old_start + 1,
        new_end.saturating_sub(old_start)
    ));
    let mut old_index = old_start;
    let mut new_index = old_start;
    while old_index < old_end || new_index < new_end {
        match (old_lines.get(old_index), new_lines.get(new_index)) {
            (Some(left), Some(right)) if left == right => {
                diff.push(' ');
                diff.push_str(left);
                old_index += 1;
                new_index += 1;
            }
            (Some(left), Some(right)) => {
                diff.push('-');
                diff.push_str(left);
                diff.push('+');
                diff.push_str(right);
                old_index += 1;
                new_index += 1;
            }
            (Some(left), None) => {
                diff.push('-');
                diff.push_str(left);
                old_index += 1;
            }
            (None, Some(right)) => {
                diff.push('+');
                diff.push_str(right);
                new_index += 1;
            }
            (None, None) => break,
        }
    }
    diff
}

fn split_keepends(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn collect_matching_files(path: &Path, cwd: &Path, pattern: &str, files: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        if path.is_dir() {
            collect_matching_files(&path, cwd, pattern, files);
        } else if matches_simple_glob(&path, pattern) {
            let rel = path.strip_prefix(cwd).unwrap_or(&path);
            files.push(format!("./{}", rel.to_string_lossy()));
        }
    }
}

fn matches_simple_glob(path: &Path, pattern: &str) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    name == pattern
}

const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".ico"];

const BINARY_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".ico", ".pdf", ".zip", ".tar", ".gz",
    ".bz2", ".xz", ".7z", ".rar", ".exe", ".dll", ".so", ".dylib", ".o", ".a", ".class", ".jar",
    ".pyc", ".pyo", ".db", ".sqlite", ".sqlite3",
];

pub const BUILTIN_TOOLS: &[ToolSummary] = &[
    ToolSummary {
        name: "browser_back",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &[],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "browser_cdp",
        toolset: "browser-cdp",
        is_async: false,
        requires_env: &[],
        parameter_names: &["frame_id", "method", "params", "target_id", "timeout"],
        required: &["method"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_click",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["ref"],
        required: &["ref"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_console",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["clear", "expression"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "browser_dialog",
        toolset: "browser-cdp",
        is_async: false,
        requires_env: &[],
        parameter_names: &["action", "dialog_id", "prompt_text"],
        required: &["action"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_get_images",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &[],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "browser_navigate",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["url"],
        required: &["url"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_press",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["key"],
        required: &["key"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_scroll",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["direction"],
        required: &["direction"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_snapshot",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["full"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "browser_type",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["ref", "text"],
        required: &["ref", "text"],
        description_present: true,
    },
    ToolSummary {
        name: "browser_vision",
        toolset: "browser",
        is_async: false,
        requires_env: &[],
        parameter_names: &["annotate", "question"],
        required: &["question"],
        description_present: true,
    },
    ToolSummary {
        name: "clarify",
        toolset: "clarify",
        is_async: false,
        requires_env: &[],
        parameter_names: &["choices", "question"],
        required: &["question"],
        description_present: true,
    },
    ToolSummary {
        name: "computer_use",
        toolset: "computer_use",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "action",
            "amount",
            "app",
            "button",
            "capture_after",
            "coordinate",
            "direction",
            "element",
            "from_coordinate",
            "from_element",
            "keys",
            "mode",
            "modifiers",
            "raise_window",
            "seconds",
            "text",
            "to_coordinate",
            "to_element",
            "value",
        ],
        required: &["action"],
        description_present: true,
    },
    ToolSummary {
        name: "cronjob",
        toolset: "cronjob",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "action",
            "context_from",
            "deliver",
            "enabled_toolsets",
            "job_id",
            "model",
            "name",
            "no_agent",
            "profile",
            "prompt",
            "repeat",
            "schedule",
            "script",
            "skills",
            "workdir",
        ],
        required: &["action"],
        description_present: true,
    },
    ToolSummary {
        name: "delegate_task",
        toolset: "delegation",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "acp_args",
            "acp_command",
            "context",
            "goal",
            "role",
            "tasks",
            "toolsets",
        ],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "discord",
        toolset: "discord",
        is_async: false,
        requires_env: &["DISCORD_BOT_TOKEN"],
        parameter_names: &[
            "action",
            "after",
            "auto_archive_duration",
            "before",
            "channel_id",
            "guild_id",
            "limit",
            "message_id",
            "name",
            "query",
            "role_id",
            "user_id",
        ],
        required: &["action"],
        description_present: true,
    },
    ToolSummary {
        name: "discord_admin",
        toolset: "discord_admin",
        is_async: false,
        requires_env: &["DISCORD_BOT_TOKEN"],
        parameter_names: &[
            "action",
            "after",
            "auto_archive_duration",
            "before",
            "channel_id",
            "guild_id",
            "limit",
            "message_id",
            "name",
            "query",
            "role_id",
            "user_id",
        ],
        required: &["action"],
        description_present: true,
    },
    ToolSummary {
        name: "execute_code",
        toolset: "code_execution",
        is_async: false,
        requires_env: &[],
        parameter_names: &["code"],
        required: &["code"],
        description_present: true,
    },
    ToolSummary {
        name: "feishu_doc_read",
        toolset: "feishu_doc",
        is_async: false,
        requires_env: &[],
        parameter_names: &["doc_token"],
        required: &["doc_token"],
        description_present: true,
    },
    ToolSummary {
        name: "feishu_drive_add_comment",
        toolset: "feishu_drive",
        is_async: false,
        requires_env: &[],
        parameter_names: &["content", "file_token", "file_type"],
        required: &["file_token", "content"],
        description_present: true,
    },
    ToolSummary {
        name: "feishu_drive_list_comment_replies",
        toolset: "feishu_drive",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "comment_id",
            "file_token",
            "file_type",
            "page_size",
            "page_token",
        ],
        required: &["file_token", "comment_id"],
        description_present: true,
    },
    ToolSummary {
        name: "feishu_drive_list_comments",
        toolset: "feishu_drive",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "file_token",
            "file_type",
            "is_whole",
            "page_size",
            "page_token",
        ],
        required: &["file_token"],
        description_present: true,
    },
    ToolSummary {
        name: "feishu_drive_reply_comment",
        toolset: "feishu_drive",
        is_async: false,
        requires_env: &[],
        parameter_names: &["comment_id", "content", "file_token", "file_type"],
        required: &["file_token", "comment_id", "content"],
        description_present: true,
    },
    ToolSummary {
        name: "ha_call_service",
        toolset: "homeassistant",
        is_async: false,
        requires_env: &[],
        parameter_names: &["data", "domain", "entity_id", "service"],
        required: &["domain", "service"],
        description_present: true,
    },
    ToolSummary {
        name: "ha_get_state",
        toolset: "homeassistant",
        is_async: false,
        requires_env: &[],
        parameter_names: &["entity_id"],
        required: &["entity_id"],
        description_present: true,
    },
    ToolSummary {
        name: "ha_list_entities",
        toolset: "homeassistant",
        is_async: false,
        requires_env: &[],
        parameter_names: &["area", "domain"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "ha_list_services",
        toolset: "homeassistant",
        is_async: false,
        requires_env: &[],
        parameter_names: &["domain"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "image_generate",
        toolset: "image_gen",
        is_async: false,
        requires_env: &[],
        parameter_names: &["aspect_ratio", "prompt"],
        required: &["prompt"],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_block",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &["board", "reason", "task_id"],
        required: &["reason"],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_comment",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &["board", "body", "task_id"],
        required: &["task_id", "body"],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_complete",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "artifacts",
            "board",
            "created_cards",
            "metadata",
            "result",
            "summary",
            "task_id",
        ],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_create",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "assignee",
            "board",
            "body",
            "idempotency_key",
            "initial_status",
            "max_runtime_seconds",
            "parents",
            "priority",
            "skills",
            "tenant",
            "title",
            "triage",
            "workspace_kind",
            "workspace_path",
        ],
        required: &["title", "assignee"],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_heartbeat",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &["board", "note", "task_id"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_link",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &["board", "child_id", "parent_id"],
        required: &["parent_id", "child_id"],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_list",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "assignee",
            "board",
            "include_archived",
            "limit",
            "status",
            "tenant",
        ],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_show",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &["board", "task_id"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "kanban_unblock",
        toolset: "kanban",
        is_async: false,
        requires_env: &[],
        parameter_names: &["board", "task_id"],
        required: &["task_id"],
        description_present: true,
    },
    ToolSummary {
        name: "memory",
        toolset: "memory",
        is_async: false,
        requires_env: &[],
        parameter_names: &["action", "content", "old_text", "target"],
        required: &["action", "target"],
        description_present: true,
    },
    ToolSummary {
        name: "mixture_of_agents",
        toolset: "moa",
        is_async: true,
        requires_env: &["OPENROUTER_API_KEY"],
        parameter_names: &["user_prompt"],
        required: &["user_prompt"],
        description_present: true,
    },
    ToolSummary {
        name: "patch",
        toolset: "file",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "mode",
            "new_string",
            "old_string",
            "patch",
            "path",
            "replace_all",
        ],
        required: &["mode"],
        description_present: true,
    },
    ToolSummary {
        name: "process",
        toolset: "terminal",
        is_async: false,
        requires_env: &[],
        parameter_names: &["action", "data", "limit", "offset", "session_id", "timeout"],
        required: &["action"],
        description_present: true,
    },
    ToolSummary {
        name: "read_file",
        toolset: "file",
        is_async: false,
        requires_env: &[],
        parameter_names: &["limit", "offset", "path"],
        required: &["path"],
        description_present: true,
    },
    ToolSummary {
        name: "search_files",
        toolset: "file",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "context",
            "file_glob",
            "limit",
            "offset",
            "output_mode",
            "path",
            "pattern",
            "target",
        ],
        required: &["pattern"],
        description_present: true,
    },
    ToolSummary {
        name: "send_message",
        toolset: "messaging",
        is_async: false,
        requires_env: &[],
        parameter_names: &["action", "message", "target"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "session_search",
        toolset: "session_search",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "around_message_id",
            "limit",
            "query",
            "role_filter",
            "session_id",
            "sort",
            "window",
        ],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "skill_manage",
        toolset: "skills",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "absorbed_into",
            "action",
            "category",
            "content",
            "file_content",
            "file_path",
            "name",
            "new_string",
            "old_string",
            "replace_all",
        ],
        required: &["action", "name"],
        description_present: true,
    },
    ToolSummary {
        name: "skill_view",
        toolset: "skills",
        is_async: false,
        requires_env: &[],
        parameter_names: &["file_path", "name"],
        required: &["name"],
        description_present: true,
    },
    ToolSummary {
        name: "skills_list",
        toolset: "skills",
        is_async: false,
        requires_env: &[],
        parameter_names: &["category"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "terminal",
        toolset: "terminal",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "background",
            "command",
            "notify_on_complete",
            "pty",
            "timeout",
            "watch_patterns",
            "workdir",
        ],
        required: &["command"],
        description_present: true,
    },
    ToolSummary {
        name: "text_to_speech",
        toolset: "tts",
        is_async: false,
        requires_env: &[],
        parameter_names: &["output_path", "text"],
        required: &["text"],
        description_present: true,
    },
    ToolSummary {
        name: "todo",
        toolset: "todo",
        is_async: false,
        requires_env: &[],
        parameter_names: &["merge", "todos"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "video_analyze",
        toolset: "video",
        is_async: true,
        requires_env: &[],
        parameter_names: &["question", "video_url"],
        required: &["video_url", "question"],
        description_present: true,
    },
    ToolSummary {
        name: "video_generate",
        toolset: "video_gen",
        is_async: false,
        requires_env: &[],
        parameter_names: &[
            "aspect_ratio",
            "audio",
            "duration",
            "image_url",
            "model",
            "negative_prompt",
            "prompt",
            "reference_image_urls",
            "resolution",
            "seed",
        ],
        required: &["prompt"],
        description_present: true,
    },
    ToolSummary {
        name: "vision_analyze",
        toolset: "vision",
        is_async: true,
        requires_env: &[],
        parameter_names: &["image_url", "question"],
        required: &["image_url", "question"],
        description_present: true,
    },
    ToolSummary {
        name: "web_extract",
        toolset: "web",
        is_async: true,
        requires_env: &[
            "EXA_API_KEY",
            "PARALLEL_API_KEY",
            "TAVILY_API_KEY",
            "FIRECRAWL_API_KEY",
            "FIRECRAWL_API_URL",
            "FIRECRAWL_GATEWAY_URL",
            "TOOL_GATEWAY_DOMAIN",
            "TOOL_GATEWAY_SCHEME",
            "TOOL_GATEWAY_USER_TOKEN",
        ],
        parameter_names: &["urls"],
        required: &["urls"],
        description_present: true,
    },
    ToolSummary {
        name: "web_search",
        toolset: "web",
        is_async: false,
        requires_env: &[
            "EXA_API_KEY",
            "PARALLEL_API_KEY",
            "TAVILY_API_KEY",
            "FIRECRAWL_API_KEY",
            "FIRECRAWL_API_URL",
            "FIRECRAWL_GATEWAY_URL",
            "TOOL_GATEWAY_DOMAIN",
            "TOOL_GATEWAY_SCHEME",
            "TOOL_GATEWAY_USER_TOKEN",
        ],
        parameter_names: &["limit", "query"],
        required: &["query"],
        description_present: true,
    },
    ToolSummary {
        name: "write_file",
        toolset: "file",
        is_async: false,
        requires_env: &[],
        parameter_names: &["content", "path"],
        required: &["path", "content"],
        description_present: true,
    },
    ToolSummary {
        name: "x_search",
        toolset: "x_search",
        is_async: false,
        requires_env: &["XAI_API_KEY"],
        parameter_names: &[
            "allowed_x_handles",
            "enable_image_understanding",
            "enable_video_understanding",
            "excluded_x_handles",
            "from_date",
            "query",
            "to_date",
        ],
        required: &["query"],
        description_present: true,
    },
    ToolSummary {
        name: "yb_query_group_info",
        toolset: "hermes-yuanbao",
        is_async: true,
        requires_env: &[],
        parameter_names: &["group_code"],
        required: &["group_code"],
        description_present: true,
    },
    ToolSummary {
        name: "yb_query_group_members",
        toolset: "hermes-yuanbao",
        is_async: true,
        requires_env: &[],
        parameter_names: &["action", "group_code", "mention", "name"],
        required: &["group_code", "action"],
        description_present: true,
    },
    ToolSummary {
        name: "yb_search_sticker",
        toolset: "hermes-yuanbao",
        is_async: true,
        requires_env: &[],
        parameter_names: &["limit", "query"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "yb_send_dm",
        toolset: "hermes-yuanbao",
        is_async: true,
        requires_env: &[],
        parameter_names: &["group_code", "media_files", "message", "name", "user_id"],
        required: &[],
        description_present: true,
    },
    ToolSummary {
        name: "yb_send_sticker",
        toolset: "hermes-yuanbao",
        is_async: true,
        requires_env: &[],
        parameter_names: &["chat_id", "reply_to", "sticker"],
        required: &[],
        description_present: true,
    },
];

const SELECTED_TOOL_PARAM_CONTRACTS: &[ToolParamContract] = &[
    ToolParamContract {
        tool: "memory",
        parameter: "action",
        json_type: "string",
        default_json: None,
        enum_values: &["add", "replace", "remove"],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "memory",
        parameter: "target",
        json_type: "string",
        default_json: None,
        enum_values: &["memory", "user"],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "patch",
        parameter: "mode",
        json_type: "string",
        default_json: Some("\"replace\""),
        enum_values: &["replace", "patch"],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "patch",
        parameter: "replace_all",
        json_type: "boolean",
        default_json: Some("false"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "read_file",
        parameter: "limit",
        json_type: "integer",
        default_json: Some("500"),
        enum_values: &[],
        minimum: None,
        maximum: Some(2000),
    },
    ToolParamContract {
        tool: "read_file",
        parameter: "offset",
        json_type: "integer",
        default_json: Some("1"),
        enum_values: &[],
        minimum: Some(1),
        maximum: None,
    },
    ToolParamContract {
        tool: "search_files",
        parameter: "context",
        json_type: "integer",
        default_json: Some("0"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "search_files",
        parameter: "limit",
        json_type: "integer",
        default_json: Some("50"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "search_files",
        parameter: "offset",
        json_type: "integer",
        default_json: Some("0"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "search_files",
        parameter: "output_mode",
        json_type: "string",
        default_json: Some("\"content\""),
        enum_values: &["content", "files_only", "count"],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "search_files",
        parameter: "path",
        json_type: "string",
        default_json: Some("\".\""),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "search_files",
        parameter: "target",
        json_type: "string",
        default_json: Some("\"content\""),
        enum_values: &["content", "files"],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "session_search",
        parameter: "around_message_id",
        json_type: "integer",
        default_json: None,
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "session_search",
        parameter: "limit",
        json_type: "integer",
        default_json: Some("3"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "session_search",
        parameter: "sort",
        json_type: "string",
        default_json: None,
        enum_values: &["newest", "oldest"],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "session_search",
        parameter: "window",
        json_type: "integer",
        default_json: Some("5"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "skill_manage",
        parameter: "action",
        json_type: "string",
        default_json: None,
        enum_values: &[
            "create",
            "patch",
            "edit",
            "delete",
            "write_file",
            "remove_file",
        ],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "skill_manage",
        parameter: "replace_all",
        json_type: "boolean",
        default_json: None,
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "terminal",
        parameter: "background",
        json_type: "boolean",
        default_json: Some("false"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "terminal",
        parameter: "notify_on_complete",
        json_type: "boolean",
        default_json: Some("false"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "terminal",
        parameter: "pty",
        json_type: "boolean",
        default_json: Some("false"),
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
    ToolParamContract {
        tool: "terminal",
        parameter: "timeout",
        json_type: "integer",
        default_json: None,
        enum_values: &[],
        minimum: Some(1),
        maximum: None,
    },
    ToolParamContract {
        tool: "terminal",
        parameter: "watch_patterns",
        json_type: "array",
        default_json: None,
        enum_values: &[],
        minimum: None,
        maximum: None,
    },
];

pub fn builtin_tools() -> &'static [ToolSummary] {
    BUILTIN_TOOLS
}

pub fn selected_tool_param_contracts() -> &'static [ToolParamContract] {
    SELECTED_TOOL_PARAM_CONTRACTS
}

pub fn file_tool_schemas_without_descriptions() -> Value {
    json!({
        "patch": {
            "name": "patch",
            "parameters": {
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["replace", "patch"],
                        "default": "replace",
                    },
                    "path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"},
                    "replace_all": {
                        "type": "boolean",
                        "default": false,
                    },
                    "patch": {"type": "string"},
                },
                "required": ["mode"],
            },
        },
        "read_file": {
            "name": "read_file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "offset": {
                        "type": "integer",
                        "default": 1,
                        "minimum": 1,
                    },
                    "limit": {
                        "type": "integer",
                        "default": 500,
                        "maximum": 2000,
                    },
                },
                "required": ["path"],
            },
        },
        "search_files": {
            "name": "search_files",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "target": {
                        "type": "string",
                        "enum": ["content", "files"],
                        "default": "content",
                    },
                    "path": {
                        "type": "string",
                        "default": ".",
                    },
                    "file_glob": {"type": "string"},
                    "limit": {
                        "type": "integer",
                        "default": 50,
                    },
                    "offset": {
                        "type": "integer",
                        "default": 0,
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["content", "files_only", "count"],
                        "default": "content",
                    },
                    "context": {
                        "type": "integer",
                        "default": 0,
                    },
                },
                "required": ["pattern"],
            },
        },
        "write_file": {
            "name": "write_file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                },
                "required": ["path", "content"],
            },
        },
    })
}

pub fn tool_by_name(name: &str) -> Option<&'static ToolSummary> {
    BUILTIN_TOOLS.iter().find(|tool| tool.name == name)
}

pub fn toolsets() -> Vec<&'static str> {
    let sets = BUILTIN_TOOLS
        .iter()
        .map(|tool| tool.toolset)
        .collect::<BTreeSet<_>>();
    sets.into_iter().collect()
}

pub fn toolset_names() -> &'static [&'static str] {
    TOOLSET_NAMES
}

pub fn validate_toolset(name: &str) -> bool {
    matches!(name, "all" | "*") || TOOLSET_NAMES.contains(&name)
}

pub fn resolve_toolset(name: &str) -> Vec<&'static str> {
    match name {
        "all" | "*" => ALL_TOOLSET_TOOLS.to_vec(),
        "web" => WEB_TOOLS.to_vec(),
        "vision" => VISION_TOOLS.to_vec(),
        "terminal" => TERMINAL_TOOLS.to_vec(),
        "file" => FILE_TOOLS.to_vec(),
        "image_gen" => IMAGE_GEN_TOOLS.to_vec(),
        "safe" => SAFE_TOOLS.to_vec(),
        "debugging" => DEBUGGING_TOOLS.to_vec(),
        "hermes-cli"
        | "hermes-cron"
        | "hermes-telegram"
        | "hermes-whatsapp"
        | "hermes-slack"
        | "hermes-signal"
        | "hermes-bluebubbles"
        | "hermes-homeassistant"
        | "hermes-email"
        | "hermes-mattermost"
        | "hermes-matrix"
        | "hermes-dingtalk"
        | "hermes-weixin"
        | "hermes-qqbot"
        | "hermes-wecom"
        | "hermes-wecom-callback"
        | "hermes-sms"
        | "hermes-webhook" => HERMES_CORE_TOOLS.to_vec(),
        "hermes-discord" => HERMES_DISCORD_TOOLS.to_vec(),
        "hermes-feishu" => HERMES_FEISHU_TOOLS.to_vec(),
        "hermes-gateway" => HERMES_GATEWAY_TOOLS.to_vec(),
        _ => Vec::new(),
    }
}

pub fn resolve_multiple_toolsets(names: &[&str]) -> Vec<&'static str> {
    let mut tools = BTreeSet::new();
    for name in names {
        tools.extend(resolve_toolset(name));
    }
    tools.into_iter().collect()
}

pub fn toolset_info(name: &str) -> Option<ToolsetInfo> {
    match name {
        "web" => Some(ToolsetInfo {
            name: "web",
            description: "Web research and content extraction tools",
            direct_tools: WEB_TOOLS,
            includes: &[],
            resolved_tools: WEB_TOOLS,
            is_composite: false,
        }),
        "safe" => Some(ToolsetInfo {
            name: "safe",
            description: "Safe toolkit without terminal access",
            direct_tools: &[],
            includes: &["web", "vision", "image_gen"],
            resolved_tools: SAFE_TOOLS,
            is_composite: true,
        }),
        "debugging" => Some(ToolsetInfo {
            name: "debugging",
            description: "Debugging and troubleshooting toolkit",
            direct_tools: TERMINAL_TOOLS,
            includes: &["web", "file"],
            resolved_tools: DEBUGGING_TOOLS,
            is_composite: true,
        }),
        "hermes-cli" => Some(ToolsetInfo {
            name: "hermes-cli",
            description: "Full interactive CLI toolset - all default tools plus cronjob management",
            direct_tools: HERMES_CORE_TOOLS,
            includes: &[],
            resolved_tools: HERMES_CORE_TOOLS,
            is_composite: false,
        }),
        "hermes-gateway" => Some(ToolsetInfo {
            name: "hermes-gateway",
            description: "Gateway toolset - union of all messaging platform tools",
            direct_tools: &[],
            includes: HERMES_GATEWAY_INCLUDES,
            resolved_tools: HERMES_GATEWAY_TOOLS,
            is_composite: true,
        }),
        _ => None,
    }
}

const WEB_TOOLS: &[&str] = &["web_extract", "web_search"];
const VISION_TOOLS: &[&str] = &["vision_analyze"];
const TERMINAL_TOOLS: &[&str] = &["process", "terminal"];
const FILE_TOOLS: &[&str] = &["patch", "read_file", "search_files", "write_file"];
const IMAGE_GEN_TOOLS: &[&str] = &["image_generate"];
const SAFE_TOOLS: &[&str] = &[
    "image_generate",
    "vision_analyze",
    "web_extract",
    "web_search",
];
const DEBUGGING_TOOLS: &[&str] = &[
    "patch",
    "process",
    "read_file",
    "search_files",
    "terminal",
    "web_extract",
    "web_search",
    "write_file",
];

const HERMES_CORE_TOOLS: &[&str] = &[
    "browser_back",
    "browser_cdp",
    "browser_click",
    "browser_console",
    "browser_dialog",
    "browser_get_images",
    "browser_navigate",
    "browser_press",
    "browser_scroll",
    "browser_snapshot",
    "browser_type",
    "browser_vision",
    "clarify",
    "computer_use",
    "cronjob",
    "delegate_task",
    "execute_code",
    "ha_call_service",
    "ha_get_state",
    "ha_list_entities",
    "ha_list_services",
    "image_generate",
    "kanban_block",
    "kanban_comment",
    "kanban_complete",
    "kanban_create",
    "kanban_heartbeat",
    "kanban_link",
    "kanban_list",
    "kanban_show",
    "kanban_unblock",
    "memory",
    "patch",
    "process",
    "read_file",
    "search_files",
    "send_message",
    "session_search",
    "skill_manage",
    "skill_view",
    "skills_list",
    "terminal",
    "text_to_speech",
    "todo",
    "vision_analyze",
    "web_extract",
    "web_search",
    "write_file",
];

const HERMES_DISCORD_TOOLS: &[&str] = &[
    "browser_back",
    "browser_cdp",
    "browser_click",
    "browser_console",
    "browser_dialog",
    "browser_get_images",
    "browser_navigate",
    "browser_press",
    "browser_scroll",
    "browser_snapshot",
    "browser_type",
    "browser_vision",
    "clarify",
    "computer_use",
    "cronjob",
    "delegate_task",
    "discord",
    "discord_admin",
    "execute_code",
    "ha_call_service",
    "ha_get_state",
    "ha_list_entities",
    "ha_list_services",
    "image_generate",
    "kanban_block",
    "kanban_comment",
    "kanban_complete",
    "kanban_create",
    "kanban_heartbeat",
    "kanban_link",
    "kanban_list",
    "kanban_show",
    "kanban_unblock",
    "memory",
    "patch",
    "process",
    "read_file",
    "search_files",
    "send_message",
    "session_search",
    "skill_manage",
    "skill_view",
    "skills_list",
    "terminal",
    "text_to_speech",
    "todo",
    "vision_analyze",
    "web_extract",
    "web_search",
    "write_file",
];

const HERMES_FEISHU_TOOLS: &[&str] = &[
    "browser_back",
    "browser_cdp",
    "browser_click",
    "browser_console",
    "browser_dialog",
    "browser_get_images",
    "browser_navigate",
    "browser_press",
    "browser_scroll",
    "browser_snapshot",
    "browser_type",
    "browser_vision",
    "clarify",
    "computer_use",
    "cronjob",
    "delegate_task",
    "execute_code",
    "feishu_doc_read",
    "feishu_drive_add_comment",
    "feishu_drive_list_comment_replies",
    "feishu_drive_list_comments",
    "feishu_drive_reply_comment",
    "ha_call_service",
    "ha_get_state",
    "ha_list_entities",
    "ha_list_services",
    "image_generate",
    "kanban_block",
    "kanban_comment",
    "kanban_complete",
    "kanban_create",
    "kanban_heartbeat",
    "kanban_link",
    "kanban_list",
    "kanban_show",
    "kanban_unblock",
    "memory",
    "patch",
    "process",
    "read_file",
    "search_files",
    "send_message",
    "session_search",
    "skill_manage",
    "skill_view",
    "skills_list",
    "terminal",
    "text_to_speech",
    "todo",
    "vision_analyze",
    "web_extract",
    "web_search",
    "write_file",
];

const HERMES_GATEWAY_TOOLS: &[&str] = &[
    "browser_back",
    "browser_cdp",
    "browser_click",
    "browser_console",
    "browser_dialog",
    "browser_get_images",
    "browser_navigate",
    "browser_press",
    "browser_scroll",
    "browser_snapshot",
    "browser_type",
    "browser_vision",
    "clarify",
    "computer_use",
    "cronjob",
    "delegate_task",
    "discord",
    "discord_admin",
    "execute_code",
    "feishu_doc_read",
    "feishu_drive_add_comment",
    "feishu_drive_list_comment_replies",
    "feishu_drive_list_comments",
    "feishu_drive_reply_comment",
    "ha_call_service",
    "ha_get_state",
    "ha_list_entities",
    "ha_list_services",
    "image_generate",
    "kanban_block",
    "kanban_comment",
    "kanban_complete",
    "kanban_create",
    "kanban_heartbeat",
    "kanban_link",
    "kanban_list",
    "kanban_show",
    "kanban_unblock",
    "memory",
    "patch",
    "process",
    "read_file",
    "search_files",
    "send_message",
    "session_search",
    "skill_manage",
    "skill_view",
    "skills_list",
    "terminal",
    "text_to_speech",
    "todo",
    "vision_analyze",
    "web_extract",
    "web_search",
    "write_file",
    "yb_query_group_info",
    "yb_query_group_members",
    "yb_search_sticker",
    "yb_send_dm",
    "yb_send_sticker",
];

const ALL_TOOLSET_TOOLS: &[&str] = &[
    "browser_back",
    "browser_cdp",
    "browser_click",
    "browser_console",
    "browser_dialog",
    "browser_get_images",
    "browser_navigate",
    "browser_press",
    "browser_scroll",
    "browser_snapshot",
    "browser_type",
    "browser_vision",
    "clarify",
    "computer_use",
    "cronjob",
    "delegate_task",
    "discord",
    "discord_admin",
    "execute_code",
    "feishu_doc_read",
    "feishu_drive_add_comment",
    "feishu_drive_list_comment_replies",
    "feishu_drive_list_comments",
    "feishu_drive_reply_comment",
    "ha_call_service",
    "ha_get_state",
    "ha_list_entities",
    "ha_list_services",
    "image_generate",
    "kanban_block",
    "kanban_comment",
    "kanban_complete",
    "kanban_create",
    "kanban_heartbeat",
    "kanban_link",
    "kanban_list",
    "kanban_show",
    "kanban_unblock",
    "memory",
    "mixture_of_agents",
    "patch",
    "process",
    "read_file",
    "search_files",
    "send_message",
    "session_search",
    "skill_manage",
    "skill_view",
    "skills_list",
    "spotify_albums",
    "spotify_devices",
    "spotify_library",
    "spotify_playback",
    "spotify_playlists",
    "spotify_queue",
    "spotify_search",
    "terminal",
    "text_to_speech",
    "todo",
    "video_analyze",
    "video_generate",
    "vision_analyze",
    "web_extract",
    "web_search",
    "write_file",
    "x_search",
    "yb_query_group_info",
    "yb_query_group_members",
    "yb_search_sticker",
    "yb_send_dm",
    "yb_send_sticker",
];

const HERMES_GATEWAY_INCLUDES: &[&str] = &[
    "hermes-telegram",
    "hermes-discord",
    "hermes-whatsapp",
    "hermes-slack",
    "hermes-signal",
    "hermes-bluebubbles",
    "hermes-homeassistant",
    "hermes-email",
    "hermes-sms",
    "hermes-mattermost",
    "hermes-matrix",
    "hermes-dingtalk",
    "hermes-feishu",
    "hermes-wecom",
    "hermes-wecom-callback",
    "hermes-weixin",
    "hermes-qqbot",
    "hermes-webhook",
    "hermes-yuanbao",
];

const TOOLSET_NAMES: &[&str] = &[
    "browser",
    "clarify",
    "code_execution",
    "computer_use",
    "cronjob",
    "debugging",
    "delegation",
    "discord",
    "discord_admin",
    "feishu_doc",
    "feishu_drive",
    "file",
    "hermes-acp",
    "hermes-api-server",
    "hermes-bluebubbles",
    "hermes-cli",
    "hermes-cron",
    "hermes-dingtalk",
    "hermes-discord",
    "hermes-email",
    "hermes-feishu",
    "hermes-gateway",
    "hermes-homeassistant",
    "hermes-matrix",
    "hermes-mattermost",
    "hermes-qqbot",
    "hermes-signal",
    "hermes-slack",
    "hermes-sms",
    "hermes-telegram",
    "hermes-webhook",
    "hermes-wecom",
    "hermes-wecom-callback",
    "hermes-weixin",
    "hermes-whatsapp",
    "hermes-yuanbao",
    "homeassistant",
    "image_gen",
    "kanban",
    "memory",
    "messaging",
    "moa",
    "safe",
    "search",
    "session_search",
    "skills",
    "spotify",
    "terminal",
    "todo",
    "tts",
    "video",
    "video_gen",
    "vision",
    "web",
    "x_search",
    "yuanbao",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let names = BUILTIN_TOOLS
            .iter()
            .map(|tool| tool.name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), BUILTIN_TOOLS.len());
    }

    #[test]
    fn registry_exposes_core_toolsets() {
        let toolsets = toolsets();
        assert!(toolsets.contains(&"memory"));
        assert!(toolsets.contains(&"skills"));
        assert!(toolsets.contains(&"terminal"));
    }
}
