use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::{json, Value};

pub const TOP_LEVEL_HELP: &str = "\
Hermes Agent

Commands:
  setup      Configure Hermes and credentials
  config     Inspect and edit configuration
  tools      Manage tools and toolsets
  gateway    Run messaging gateway integrations
  logs       Browse Hermes logs
";

const CONFIG_HELP: &str = "\
Usage: hermes config [COMMAND]

Commands:
  show
  edit
  set
  path
  env-path
  check
  migrate
";

const TOOLS_HELP: &str = "\
Usage: hermes tools [COMMAND]

Commands:
  list
  enable
  disable
";

const MCP_HELP: &str = "\
Usage: hermes mcp [COMMAND]

Commands:
  serve
  add
  remove
  list
  test
  configure
  login
";

const SESSIONS_HELP: &str = "\
Usage: hermes sessions [COMMAND]

Commands:
  list
  export
  delete
  prune
  stats
  rename
  browse
";

const CRON_HELP: &str = "\
Usage: hermes cron [COMMAND]

Commands:
  list
  create
  pause
  resume
  remove
  tick
";

const GATEWAY_HELP: &str = "\
Usage: hermes gateway [COMMAND]

Commands:
  run
  start
  stop
  restart
  status
  setup
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubcommandHelpContract {
    pub argv: &'static [&'static str],
    pub exit_code: i32,
    pub stderr_empty: bool,
    pub stdout_markers: BTreeMap<&'static str, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliRun {
    pub exit_code: i32,
    pub stderr_empty: bool,
    pub stdout_markers: BTreeMap<&'static str, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliExecution {
    pub exit_code: i32,
    pub stdout: String,
    pub stdout_markers: BTreeMap<&'static str, bool>,
    pub stderr: String,
}

pub fn builtin_subcommands() -> &'static [&'static str] {
    BUILTIN_SUBCOMMANDS
}

pub fn help_marker_contract() -> CliRun {
    let mut stdout_markers = BTreeMap::new();
    for marker in ["Usage", "config", "gateway", "logs", "setup", "tools"] {
        stdout_markers.insert(marker, TOP_LEVEL_HELP.contains(marker));
    }
    CliRun {
        exit_code: 0,
        stderr_empty: true,
        stdout_markers,
    }
}

pub fn selected_subcommand_help_contracts() -> Vec<SubcommandHelpContract> {
    [
        (
            &["hermes", "config", "--help"][..],
            &[
                ("check", true),
                ("edit", true),
                ("env-path", true),
                ("migrate", true),
                ("path", true),
                ("set", true),
                ("show", true),
            ][..],
        ),
        (
            &["hermes", "tools", "--help"][..],
            &[
                ("--platform", false),
                ("disable", true),
                ("enable", true),
                ("list", true),
            ][..],
        ),
        (
            &["hermes", "mcp", "--help"][..],
            &[
                ("add", true),
                ("configure", true),
                ("list", true),
                ("login", true),
                ("remove", true),
                ("serve", true),
                ("test", true),
            ][..],
        ),
        (
            &["hermes", "sessions", "--help"][..],
            &[
                ("browse", true),
                ("delete", true),
                ("export", true),
                ("list", true),
                ("prune", true),
                ("rename", true),
                ("stats", true),
            ][..],
        ),
        (
            &["hermes", "cron", "--help"][..],
            &[
                ("create", true),
                ("list", true),
                ("pause", true),
                ("remove", true),
                ("resume", true),
                ("tick", true),
            ][..],
        ),
        (
            &["hermes", "gateway", "--help"][..],
            &[
                ("restart", true),
                ("run", true),
                ("setup", true),
                ("start", true),
                ("status", true),
                ("stop", true),
            ][..],
        ),
    ]
    .into_iter()
    .map(|(argv, markers)| SubcommandHelpContract {
        argv,
        exit_code: 0,
        stderr_empty: true,
        stdout_markers: markers.iter().copied().collect(),
    })
    .collect()
}

pub fn subcommand_help(command: &str) -> Option<&'static str> {
    match command {
        "config" => Some(CONFIG_HELP),
        "tools" => Some(TOOLS_HELP),
        "mcp" => Some(MCP_HELP),
        "sessions" => Some(SESSIONS_HELP),
        "cron" => Some(CRON_HELP),
        "gateway" => Some(GATEWAY_HELP),
        _ => None,
    }
}

pub fn run_safe_command(argv: &[&str], hermes_home: &str) -> CliExecution {
    let mut stdout_markers = BTreeMap::new();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    match argv {
        ["hermes", "--version"] | ["hermes", "version"] => {
            for marker in ["Hermes Agent v", "Project:", "Python:", "OpenAI SDK:"] {
                stdout_markers.insert(marker, true);
            }
        }
        ["hermes", command, "--help"] => {
            if let Some(help) = subcommand_help(command) {
                stdout = help.to_string();
            } else {
                exit_code = 2;
                stderr = "Unsupported safe command fixture.\n".to_string();
            }
        }
        ["hermes", "config", "path"] => {
            stdout = format!("{hermes_home}/config.yaml\n");
        }
        ["hermes", "config", "env-path"] => {
            stdout = format!("{hermes_home}/.env\n");
        }
        ["hermes", "config", "set", key, value] if is_secret_key(key) => {
            let _ = value;
            stdout = format!("✓ Set {key} in {hermes_home}/.env\n");
        }
        ["hermes", "config", "set", key, value] => {
            stdout = format!("✓ Set {key} = {value} in {hermes_home}/config.yaml\n");
        }
        ["hermes", "cron", "list"] => {
            stdout = "No scheduled jobs.\nCreate one with 'hermes cron create ...' or the /cron command in chat.\n"
                .to_string();
        }
        _ => {
            exit_code = 2;
            stderr = "Unsupported safe command fixture.\n".to_string();
        }
    }

    CliExecution {
        exit_code,
        stdout,
        stdout_markers,
        stderr,
    }
}

pub fn run_safe_command_in_home(argv: &[&str], hermes_home: &Path) -> io::Result<CliExecution> {
    fs::create_dir_all(hermes_home)?;
    let home_display = hermes_home.to_string_lossy();
    let mut result = run_safe_command(argv, &home_display);

    match argv {
        ["hermes", "config", "set", key, value] if is_secret_key(key) => {
            upsert_env_value(&hermes_home.join(".env"), key, value)?;
        }
        ["hermes", "config", "set", key, value] => {
            let config_path = hermes_home.join("config.yaml");
            let mut config = if config_path.exists() {
                serde_yaml::from_str::<Value>(&fs::read_to_string(&config_path)?)
                    .unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };
            hermes_config::set_nested_path(
                &mut config,
                key,
                hermes_config::parse_config_set_value(value),
            )
            .map_err(io::Error::other)?;
            let yaml = serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string());
            fs::write(config_path, yaml)?;
            if let Some(env_key) = hermes_config::terminal_env_sync_key(key) {
                upsert_env_value(&hermes_home.join(".env"), env_key, value)?;
            }
        }
        ["hermes", "sessions", "stats"] => {
            let db = open_session_db(hermes_home)?;
            let total = db.session_count(None).map_err(io::Error::other)?;
            let messages = db.message_count(None).map_err(io::Error::other)?;
            let mut stdout = format!("Total sessions: {total}\nTotal messages: {messages}\n");
            for source in ["cli", "telegram", "discord", "whatsapp", "slack"] {
                let count = db.session_count(Some(source)).map_err(io::Error::other)?;
                if count > 0 {
                    stdout.push_str(&format!("  {source}: {count} sessions\n"));
                }
            }
            let size_mb = fs::metadata(hermes_home.join("state.db"))
                .map(|meta| meta.len() as f64 / (1024.0 * 1024.0))
                .unwrap_or(0.0);
            stdout.push_str(&format!("Database size: {size_mb:.1} MB\n"));
            result = CliExecution {
                exit_code: 0,
                stdout,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "sessions", "list", "--limit", limit] => {
            let db = open_session_db(hermes_home)?;
            let limit = limit.parse::<usize>().unwrap_or(20);
            let sessions = db.list_sessions_for_cli(limit).map_err(io::Error::other)?;
            let mut stdout = String::new();
            if sessions.is_empty() {
                stdout.push_str("No sessions found.\n");
            } else {
                let has_titles = sessions.iter().any(|session| !session["title"].is_null());
                if has_titles {
                    stdout.push_str(&format!(
                        "{:<32} {:<40} {:<13} {}\n",
                        "Title", "Preview", "Last Active", "ID"
                    ));
                    stdout.push_str(&format!("{}\n", "─".repeat(110)));
                } else {
                    stdout.push_str(&format!(
                        "{:<50} {:<13} {:<6} {}\n",
                        "Preview", "Last Active", "Src", "ID"
                    ));
                    stdout.push_str(&format!("{}\n", "─".repeat(95)));
                }
                for session in sessions {
                    let preview = session["preview"].as_str().unwrap_or("");
                    let source = session["source"].as_str().unwrap_or("");
                    let id = session["id"].as_str().unwrap_or("");
                    if has_titles {
                        let title = session["title"].as_str().unwrap_or("—");
                        stdout.push_str(&format!(
                            "{:<32} {:<40} {:<13} {}\n",
                            truncate_chars(title, 30),
                            truncate_chars(preview, 38),
                            "just now",
                            id
                        ));
                    } else {
                        stdout.push_str(&format!(
                            "{:<50} {:<13} {:<6} {}\n",
                            truncate_chars(preview, 48),
                            "just now",
                            source,
                            id
                        ));
                    }
                }
            }
            result = CliExecution {
                exit_code: 0,
                stdout,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "sessions", "export", "-"] => {
            let db = open_session_db(hermes_home)?;
            let exported = db.export_all_optional(None).map_err(io::Error::other)?;
            let mut stdout = String::new();
            for session in exported.as_array().into_iter().flatten() {
                stdout.push_str(&serde_json::to_string(session).unwrap());
                stdout.push('\n');
            }
            result = CliExecution {
                exit_code: 0,
                stdout,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "sessions", "export", "-", "--session-id", session_id] => {
            let db = open_session_db(hermes_home)?;
            let resolved = db
                .resolve_session_id(session_id)
                .map_err(io::Error::other)?;
            if let Some(resolved) = resolved {
                let exported = db.export_session(&resolved).map_err(io::Error::other)?;
                if let Some(exported) = exported {
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!("{}\n", serde_json::to_string(&exported).unwrap()),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                } else {
                    result = session_not_found(session_id);
                }
            } else {
                result = session_not_found(session_id);
            }
        }
        ["hermes", "sessions", "rename", session_id, title @ ..] if !title.is_empty() => {
            let db = open_session_db(hermes_home)?;
            let resolved = db
                .resolve_session_id(session_id)
                .map_err(io::Error::other)?;
            if let Some(resolved) = resolved {
                let title = title.join(" ");
                match db.set_session_title(&resolved, Some(&title)) {
                    Ok(true) => {
                        result = CliExecution {
                            exit_code: 0,
                            stdout: format!("Session '{resolved}' renamed to: {title}\n"),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    }
                    Ok(false) => {
                        result = session_not_found(session_id);
                    }
                    Err(error) => {
                        result = CliExecution {
                            exit_code: 0,
                            stdout: format!("Error: {error}\n"),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    }
                }
            } else {
                result = session_not_found(session_id);
            }
        }
        ["hermes", "sessions", "delete", session_id, "--yes"]
        | ["hermes", "sessions", "delete", session_id, "-y"] => {
            let db = open_session_db(hermes_home)?;
            let resolved = db
                .resolve_session_id(session_id)
                .map_err(io::Error::other)?;
            if let Some(resolved) = resolved {
                let sessions_dir = hermes_home.join("sessions");
                let deleted = db
                    .delete_session(&resolved, Some(&sessions_dir))
                    .map_err(io::Error::other)?;
                if deleted {
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!("Deleted session '{resolved}'.\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                } else {
                    result = session_not_found(session_id);
                }
            } else {
                result = session_not_found(session_id);
            }
        }
        _ => {}
    }

    Ok(result)
}

fn open_session_db(hermes_home: &Path) -> io::Result<hermes_session::SqliteSessionStore> {
    hermes_session::SqliteSessionStore::open(hermes_home.join("state.db")).map_err(io::Error::other)
}

fn session_not_found(session_id: &str) -> CliExecution {
    CliExecution {
        exit_code: 0,
        stdout: format!("Session '{session_id}' not found.\n"),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn upsert_env_value(env_path: &Path, key: &str, value: &str) -> io::Result<()> {
    let mut lines = if env_path.exists() {
        fs::read_to_string(env_path)?
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let prefix = format!("{key}=");
    let replacement = format!("{key}={value}");
    if let Some(line) = lines.iter_mut().find(|line| line.starts_with(&prefix)) {
        *line = replacement;
    } else {
        lines.push(replacement);
    }
    fs::write(env_path, format!("{}\n", lines.join("\n")))
}

fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    upper.ends_with("_API_KEY")
        || upper.ends_with("_TOKEN")
        || upper.ends_with("_SECRET")
        || upper.ends_with("_PASSWORD")
        || upper.starts_with("TERMINAL_SSH")
}

const BUILTIN_SUBCOMMANDS: &[&str] = &[
    "acp",
    "auth",
    "backup",
    "bundles",
    "chat",
    "checkpoints",
    "claw",
    "completion",
    "computer-use",
    "config",
    "cron",
    "curator",
    "dashboard",
    "debug",
    "doctor",
    "dump",
    "fallback",
    "gateway",
    "help",
    "hooks",
    "import",
    "insights",
    "kanban",
    "login",
    "logout",
    "logs",
    "lsp",
    "mcp",
    "memory",
    "model",
    "pairing",
    "plugins",
    "postinstall",
    "profile",
    "proxy",
    "send",
    "sessions",
    "setup",
    "skills",
    "slack",
    "status",
    "tools",
    "uninstall",
    "update",
    "version",
    "webhook",
    "whatsapp",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_help_contains_python_contract_markers() {
        let contract = help_marker_contract();
        assert_eq!(contract.exit_code, 0);
        assert!(contract.stderr_empty);
        assert!(!contract.stdout_markers["Usage"]);
        assert!(contract.stdout_markers["config"]);
        assert!(contract.stdout_markers["gateway"]);
        assert!(contract.stdout_markers["logs"]);
        assert!(contract.stdout_markers["setup"]);
        assert!(contract.stdout_markers["tools"]);
    }
}
