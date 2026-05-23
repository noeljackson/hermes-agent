use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDef {
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub aliases: &'static [&'static str],
    pub args_hint: &'static str,
    pub cli_only: bool,
    pub gateway_only: bool,
    pub gateway_config_gate: Option<&'static str>,
}

pub const COMMAND_REGISTRY: &[CommandDef] = &[
    CommandDef {
        name: "agents",
        description: "Show active agents and running tasks",
        category: "Session",
        aliases: &["tasks"],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "approve",
        description: "Approve a pending dangerous command",
        category: "Session",
        aliases: &[],
        args_hint: "[session|always]",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "background",
        description: "Run a prompt in the background",
        category: "Session",
        aliases: &["bg", "btw"],
        args_hint: "<prompt>",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "branch",
        description: "Branch the current session (explore a different path)",
        category: "Session",
        aliases: &["fork"],
        args_hint: "[name]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "browser",
        description: "Connect browser tools to your live Chromium-family browser via CDP",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "[connect|disconnect|status]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "bundles",
        description: "List skill bundles (aliases /<name> for multiple skills)",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "busy",
        description: "Control what Enter does while Hermes is working",
        category: "Configuration",
        aliases: &[],
        args_hint: "[queue|steer|interrupt|status]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "clear",
        description: "Clear screen and start a new session",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "codex-runtime",
        description: "Toggle codex app-server runtime for OpenAI/Codex models",
        category: "Configuration",
        aliases: &["codex_runtime"],
        args_hint: "[auto|codex_app_server]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "commands",
        description: "Browse all commands and skills (paginated)",
        category: "Info",
        aliases: &[],
        args_hint: "[page]",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "compress",
        description: "Manually compress conversation context",
        category: "Session",
        aliases: &[],
        args_hint: "[focus topic]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "config",
        description: "Show current configuration",
        category: "Configuration",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "copy",
        description: "Copy the last assistant response to clipboard",
        category: "Info",
        aliases: &[],
        args_hint: "[number]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "cron",
        description: "Manage scheduled tasks",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "[subcommand]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "curator",
        description: "Background skill maintenance (status, run, pin, archive, list-archived)",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "[subcommand]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "debug",
        description: "Upload debug report (system info + logs) and get shareable links",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "deny",
        description: "Deny a pending dangerous command",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "fast",
        description:
            "Toggle fast mode — OpenAI Priority Processing / Anthropic Fast Mode (Normal/Fast)",
        category: "Configuration",
        aliases: &[],
        args_hint: "[normal|fast|status]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "footer",
        description: "Toggle gateway runtime-metadata footer on final replies",
        category: "Configuration",
        aliases: &[],
        args_hint: "[on|off|status]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "goal",
        description: "Set a standing goal Hermes works on across turns until achieved",
        category: "Session",
        aliases: &[],
        args_hint: "[text | pause | resume | clear | status]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "gquota",
        description: "Show Google Gemini Code Assist quota usage",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "handoff",
        description: "Hand off this session to a messaging platform (Telegram, Discord, etc.)",
        category: "Session",
        aliases: &[],
        args_hint: "<platform>",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "help",
        description: "Show available commands",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "history",
        description: "Show conversation history",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "image",
        description: "Attach a local image file for your next prompt",
        category: "Info",
        aliases: &[],
        args_hint: "<path>",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "indicator",
        description: "Pick the TUI busy-indicator style",
        category: "Configuration",
        aliases: &[],
        args_hint: "[kaomoji|emoji|unicode|ascii]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "insights",
        description: "Show usage insights and analytics",
        category: "Info",
        aliases: &[],
        args_hint: "[days]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "kanban",
        description: "Multi-profile collaboration board (tasks, links, comments)",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "[subcommand]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "model",
        description: "Switch model for this session",
        category: "Configuration",
        aliases: &["provider"],
        args_hint: "[model] [--provider name] [--global]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "new",
        description: "Start a new session (fresh session ID + history)",
        category: "Session",
        aliases: &["reset"],
        args_hint: "[name]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "paste",
        description: "Attach clipboard image from your clipboard",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "personality",
        description: "Set a predefined personality",
        category: "Configuration",
        aliases: &[],
        args_hint: "[name]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "platform",
        description: "Pause, resume, or list a failing gateway platform",
        category: "Info",
        aliases: &[],
        args_hint: "<pause|resume|list> [name]",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "platforms",
        description: "Show gateway/messaging platform status",
        category: "Info",
        aliases: &["gateway"],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "plugins",
        description: "List installed plugins and their status",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "profile",
        description: "Show active profile name and home directory",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "queue",
        description: "Queue a prompt for the next turn (doesn't interrupt)",
        category: "Session",
        aliases: &["q"],
        args_hint: "<prompt>",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "quit",
        description: "Exit the CLI (use --delete to also remove session history)",
        category: "Exit",
        aliases: &["exit"],
        args_hint: "[--delete]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "reasoning",
        description: "Manage reasoning effort and display",
        category: "Configuration",
        aliases: &[],
        args_hint: "[level|show|hide]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "redraw",
        description: "Force a full UI repaint (recovers from terminal drift)",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "reload",
        description: "Reload .env variables into the running session",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "reload-mcp",
        description: "Reload MCP servers from config",
        category: "Tools & Skills",
        aliases: &["reload_mcp"],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "reload-skills",
        description: "Re-scan ~/.hermes/skills/ for newly installed or removed skills",
        category: "Tools & Skills",
        aliases: &["reload_skills"],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "restart",
        description: "Gracefully restart the gateway after draining active runs",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "resume",
        description: "Resume a previously-named session",
        category: "Session",
        aliases: &[],
        args_hint: "[name]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "retry",
        description: "Retry the last message (resend to agent)",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "rollback",
        description: "List or restore filesystem checkpoints",
        category: "Session",
        aliases: &[],
        args_hint: "[number]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "save",
        description: "Save the current conversation",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "sessions",
        description: "Browse and resume previous sessions",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "sethome",
        description: "Set this chat as the home channel",
        category: "Session",
        aliases: &["set-home"],
        args_hint: "",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "skills",
        description: "Search, install, inspect, or manage skills",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "skin",
        description: "Show or change the display skin/theme",
        category: "Configuration",
        aliases: &[],
        args_hint: "[name]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "snapshot",
        description: "Create or restore state snapshots of Hermes config/state",
        category: "Session",
        aliases: &["snap"],
        args_hint: "[create|restore <id>|prune]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "status",
        description: "Show session info",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "statusbar",
        description: "Toggle the context/model status bar",
        category: "Configuration",
        aliases: &["sb"],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "steer",
        description: "Inject a message after the next tool call without interrupting",
        category: "Session",
        aliases: &[],
        args_hint: "<prompt>",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "stop",
        description: "Kill all running background processes",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "subgoal",
        description: "Add or manage extra criteria on the active goal",
        category: "Session",
        aliases: &[],
        args_hint: "[text | remove N | clear]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "title",
        description: "Set a title for the current session",
        category: "Session",
        aliases: &[],
        args_hint: "[name]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "tools",
        description: "Manage tools: /tools [list|disable|enable] [name...]",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "[list|disable|enable] [name...]",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "toolsets",
        description: "List available toolsets",
        category: "Tools & Skills",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "topic",
        description: "Enable or inspect Telegram DM topic sessions",
        category: "Session",
        aliases: &[],
        args_hint: "[off|help|session-id]",
        cli_only: false,
        gateway_only: true,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "undo",
        description: "Remove the last user/assistant exchange",
        category: "Session",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "update",
        description: "Update Hermes Agent to the latest version",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "usage",
        description: "Show token usage and rate limits for the current session",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "verbose",
        description: "Cycle tool progress display: off -> new -> all -> verbose",
        category: "Configuration",
        aliases: &[],
        args_hint: "",
        cli_only: true,
        gateway_only: false,
        gateway_config_gate: Some("display.tool_progress_command"),
    },
    CommandDef {
        name: "voice",
        description: "Toggle voice mode",
        category: "Configuration",
        aliases: &[],
        args_hint: "[on|off|tts|status]",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "whoami",
        description: "Show your slash command access (admin / user)",
        category: "Info",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
    CommandDef {
        name: "yolo",
        description: "Toggle YOLO mode (skip all dangerous command approvals)",
        category: "Configuration",
        aliases: &[],
        args_hint: "",
        cli_only: false,
        gateway_only: false,
        gateway_config_gate: None,
    },
];

const GATEWAY_SURFACE_ORDER: &[&str] = &[
    "new",
    "topic",
    "retry",
    "undo",
    "title",
    "branch",
    "compress",
    "rollback",
    "stop",
    "approve",
    "deny",
    "background",
    "agents",
    "queue",
    "steer",
    "goal",
    "subgoal",
    "status",
    "whoami",
    "profile",
    "sethome",
    "resume",
    "sessions",
    "model",
    "codex-runtime",
    "personality",
    "footer",
    "yolo",
    "reasoning",
    "fast",
    "voice",
    "bundles",
    "curator",
    "kanban",
    "reload-mcp",
    "reload-skills",
    "commands",
    "help",
    "restart",
    "usage",
    "insights",
    "platform",
    "update",
    "debug",
];

pub fn commands() -> &'static [CommandDef] {
    COMMAND_REGISTRY
}

pub fn command_by_name(name: &str) -> Option<&'static CommandDef> {
    let needle = normalize_command(name);
    COMMAND_REGISTRY
        .iter()
        .find(|command| command.name.eq_ignore_ascii_case(&needle))
}

pub fn resolve_command(command: &str) -> Option<&'static str> {
    let needle = normalize_command(command);
    for command in COMMAND_REGISTRY {
        if command.name.eq_ignore_ascii_case(&needle)
            || command
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&needle))
        {
            return Some(command.name);
        }
    }
    None
}

pub fn gateway_known_commands() -> Vec<&'static str> {
    let mut names = BTreeSet::new();
    for command in COMMAND_REGISTRY
        .iter()
        .filter(|command| !command.cli_only || command.gateway_config_gate.is_some())
    {
        names.insert(command.name);
        names.extend(command.aliases.iter().copied());
    }
    names.into_iter().collect()
}

pub fn active_session_bypass_commands() -> Vec<&'static str> {
    let mut names = vec![
        "agents",
        "approve",
        "background",
        "commands",
        "deny",
        "help",
        "new",
        "profile",
        "queue",
        "restart",
        "status",
        "steer",
        "stop",
        "update",
    ];
    names.sort_unstable();
    names
}

pub fn should_bypass_active_session(command: &str) -> bool {
    resolve_command(command).is_some()
}

pub fn gateway_help_lines() -> Vec<String> {
    gateway_surface_commands()
        .filter(|command| is_gateway_available(command))
        .map(|command| {
            let args = if command.args_hint.is_empty() {
                String::new()
            } else {
                format!(" {}", command.args_hint)
            };
            let aliases = command
                .aliases
                .iter()
                .filter(|alias| {
                    alias.replace('-', "_") != command.name.replace('-', "_")
                        || **alias == command.name
                })
                .map(|alias| format!("`/{alias}`"))
                .collect::<Vec<_>>();
            let alias_note = if aliases.is_empty() {
                String::new()
            } else {
                format!(" (alias: {})", aliases.join(", "))
            };
            format!(
                "`/{}{}` -- {}{}",
                command.name, args, command.description, alias_note
            )
        })
        .collect()
}

pub fn telegram_bot_commands() -> Vec<(String, &'static str)> {
    gateway_surface_commands()
        .filter(|command| is_gateway_available(command))
        .filter_map(|command| {
            sanitize_telegram_name(command.name).map(|name| (name, command.description))
        })
        .collect()
}

pub fn slack_subcommand_map() -> BTreeMap<&'static str, String> {
    let mut mapping = BTreeMap::new();
    for command in COMMAND_REGISTRY
        .iter()
        .filter(|command| is_gateway_available(command))
    {
        mapping.insert(command.name, format!("/{}", command.name));
        for alias in command.aliases {
            mapping.insert(*alias, format!("/{alias}"));
        }
    }
    mapping
}

pub fn slack_native_slashes() -> Vec<(String, String, String)> {
    const MAX_SLASH_COMMANDS: usize = 50;
    let mut entries = vec![(
        "hermes".to_string(),
        "Talk to Hermes or run a subcommand".to_string(),
        "[subcommand] [args]".to_string(),
    )];
    let mut seen = BTreeSet::from(["hermes".to_string()]);

    for command in gateway_surface_commands().filter(|command| is_gateway_available(command)) {
        add_slack_native_slash(
            &mut entries,
            &mut seen,
            MAX_SLASH_COMMANDS,
            command.name,
            command.description,
            command.args_hint,
        );
    }
    for command in gateway_surface_commands().filter(|command| is_gateway_available(command)) {
        for alias in command.aliases {
            add_slack_native_slash(
                &mut entries,
                &mut seen,
                MAX_SLASH_COMMANDS,
                alias,
                &format!("Alias for /{} -- {}", command.name, command.description),
                command.args_hint,
            );
        }
    }
    entries
}

fn add_slack_native_slash(
    entries: &mut Vec<(String, String, String)>,
    seen: &mut BTreeSet<String>,
    max_commands: usize,
    name: &str,
    description: &str,
    usage_hint: &str,
) {
    if entries.len() >= max_commands {
        return;
    }
    let Some(slack_name) = sanitize_slack_name(name) else {
        return;
    };
    if seen.contains(&slack_name) || is_slack_reserved_command(&slack_name) {
        return;
    }
    entries.push((
        slack_name.clone(),
        truncate_chars(description, 140),
        truncate_chars(usage_hint, 100),
    ));
    seen.insert(slack_name);
}

fn sanitize_slack_name(raw: &str) -> Option<String> {
    let name = raw
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || *ch == '-' || *ch == '_')
        .collect::<String>()
        .trim_matches(['-', '_'])
        .chars()
        .take(32)
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn is_slack_reserved_command(name: &str) -> bool {
    matches!(
        name,
        "me" | "status"
            | "away"
            | "dnd"
            | "shrug"
            | "remind"
            | "msg"
            | "feed"
            | "who"
            | "collapse"
            | "expand"
            | "leave"
            | "join"
            | "open"
            | "search"
            | "topic"
            | "mute"
            | "pro"
            | "shortcuts"
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn is_gateway_available(command: &CommandDef) -> bool {
    !command.cli_only
}

fn gateway_surface_commands() -> impl Iterator<Item = &'static CommandDef> {
    GATEWAY_SURFACE_ORDER.iter().filter_map(|name| {
        COMMAND_REGISTRY
            .iter()
            .find(|command| command.name == *name)
    })
}

fn sanitize_telegram_name(name: &str) -> Option<String> {
    let mut out = String::new();
    let mut last_underscore = false;
    for ch in name.to_ascii_lowercase().replace('-', "_").chars() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_';
        if !valid {
            continue;
        }
        if ch == '_' {
            if last_underscore {
                continue;
            }
            last_underscore = true;
        } else {
            last_underscore = false;
        }
        out.push(ch);
    }
    let out = out.trim_matches('_').chars().take(32).collect::<String>();
    (!out.is_empty()).then_some(out)
}

fn normalize_command(command: &str) -> String {
    let token = command
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("");
    token
        .split_once('@')
        .map_or_else(|| token, |(command, _)| command)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn registry_names_are_unique() {
        let names = COMMAND_REGISTRY
            .iter()
            .map(|command| command.name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), COMMAND_REGISTRY.len());
    }

    #[test]
    fn resolves_canonical_names_aliases_and_mentions() {
        assert_eq!(resolve_command("help"), Some("help"));
        assert_eq!(resolve_command("/q"), Some("queue"));
        assert_eq!(resolve_command("exit"), Some("quit"));
        assert_eq!(resolve_command("/help@HermesBot"), Some("help"));
        assert_eq!(resolve_command("h"), None);
    }
}
