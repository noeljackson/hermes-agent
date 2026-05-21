use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::{json, Value};
use tar::{Archive, Builder, EntryType};

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

const DEFAULT_CLI_SAVED_TOOLSETS: &[&str] = &[
    "clarify",
    "code_execution",
    "computer_use",
    "cronjob",
    "delegation",
    "file",
    "image_gen",
    "kanban",
    "memory",
    "messaging",
    "session_search",
    "skills",
    "terminal",
    "todo",
    "tts",
    "vision",
    "web",
];

const DISPLAY_TOOLSETS: &[(&str, &str, bool)] = &[
    ("web", "🔍 Web Search & Scraping", true),
    ("browser", "🌐 Browser Automation", true),
    ("terminal", "💻 Terminal & Processes", true),
    ("file", "📁 File Operations", true),
    ("code_execution", "⚡ Code Execution", true),
    ("vision", "👁️  Vision / Image Analysis", true),
    ("video", "🎬 Video Analysis", false),
    ("image_gen", "🎨 Image Generation", true),
    ("video_gen", "🎬 Video Generation", false),
    ("x_search", "🐦 X (Twitter) Search", false),
    ("moa", "🧠 Mixture of Agents", false),
    ("tts", "🔊 Text-to-Speech", true),
    ("skills", "📚 Skills", true),
    ("todo", "📋 Task Planning", true),
    ("memory", "💾 Memory", true),
    ("session_search", "🔎 Session Search", true),
    ("clarify", "❓ Clarifying Questions", true),
    ("delegation", "👥 Task Delegation", true),
    ("cronjob", "⏰ Cron Jobs", true),
    ("messaging", "📨 Cross-Platform Messaging", true),
    ("homeassistant", "🏠 Home Assistant", false),
    ("spotify", "🎵 Spotify", false),
    ("yuanbao", "🤖 Yuanbao", false),
    ("computer_use", "🖱️  Computer Use (macOS)", true),
];

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
        ["hermes", "config", "check"] => {
            stdout = "\nConfiguration Status\n\n  Config version: 23 ✓\n\n  Required:\n\n  Optional:\n    ○ OPENROUTER_API_KEY → vision_analyze, mixture_of_agents\n\n".to_string();
        }
        ["hermes", "config", "show"] => {
            stdout = format!(
                "\n┌─────────────────────────────────────────────────────────┐\n│              ⚕ Hermes Configuration                    │\n└─────────────────────────────────────────────────────────┘\n\n◆ Paths\n  Config:       {hermes_home}/config.yaml\n  Secrets:      {hermes_home}/.env\n  Install:      <PROJECT_ROOT>\n\n◆ API Keys\n  OpenRouter     <redacted>\n\n◆ Model\n  Model:        \n  Max turns:    90\n\n◆ Terminal\n  Backend:      local\n  Working dir:  .\n  Timeout:      42s\n\n◆ Context Compression\n  Enabled:      yes\n\n◆ Messaging Platforms\n  Telegram:     not configured\n  Discord:      not configured\n\n"
            );
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
        ["hermes", "cron", "create", schedule, prompt, "--name", name, "--deliver", _deliver] => {
            let display = hermes_cron::parse_schedule(schedule)
                .ok()
                .and_then(|value| {
                    value
                        .get("display")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| (*schedule).to_string());
            stdout = format!(
                "Created job: rust-{name}\n  Name: {name}\n  Schedule: {display}\n  Next run: {schedule}\n"
            );
            let _ = prompt;
        }
        ["hermes", "cron", "pause", name] => {
            stdout = format!("Paused job: {name} ({name})\n");
        }
        ["hermes", "cron", "resume", name] => {
            stdout =
                format!("Resumed job: {name} ({name})\n  Next run: 2026-06-01T09:00:00+00:00\n");
        }
        ["hermes", "cron", "remove", name]
        | ["hermes", "cron", "rm", name]
        | ["hermes", "cron", "delete", name] => {
            stdout = format!("Removed job: {name} ({name})\n");
        }
        ["hermes", "mcp", "list"] => {
            stdout = "\n  No MCP servers configured.\n\n  Add one with:\n    hermes mcp add <name> --url <endpoint>\n    hermes mcp add <name> --command <cmd> --args <args...>\n\n".to_string();
        }
        ["hermes", "tools", "list"] => {
            stdout = tools_list_output(&default_display_enabled_toolsets());
        }
        ["hermes", "tools", "enable", names @ ..] if !names.is_empty() => {
            stdout = format!("✓ Enabled: {}\n", names.join(", "));
        }
        ["hermes", "tools", "disable", names @ ..] if !names.is_empty() => {
            stdout = format!("✓ Disabled: {}\n", names.join(", "));
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
        ["hermes", "cron", "create", schedule, prompt, "--name", name, "--deliver", deliver] => {
            create_cron_job(hermes_home, schedule, prompt, name, deliver)?;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "cron", "pause", name] => {
            set_cron_job_enabled(hermes_home, name, false)?;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "cron", "resume", name] => {
            set_cron_job_enabled(hermes_home, name, true)?;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "cron", "remove", name]
        | ["hermes", "cron", "rm", name]
        | ["hermes", "cron", "delete", name] => {
            remove_cron_job(hermes_home, name)?;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "cron", "list", "--all"] | ["hermes", "cron", "list"] => {
            let jobs = hermes_cron::load_jobs(cron_jobs_path(hermes_home))?;
            result = CliExecution {
                exit_code: 0,
                stdout: cron_list_output(&jobs),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "tools", "enable", names @ ..] if !names.is_empty() => {
            apply_cli_toolset_change(hermes_home, names, true)?;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "tools", "disable", names @ ..] if !names.is_empty() => {
            apply_cli_toolset_change(hermes_home, names, false)?;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "tools", "list"] => {
            let enabled = read_cli_toolsets(hermes_home)?;
            result = CliExecution {
                exit_code: 0,
                stdout: tools_list_output(&enabled),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile"] => {
            let active = active_profile_name(hermes_home);
            let profile_dir = profile_dir(hermes_home, &active);
            let skills = count_profile_skills(&profile_dir);
            result = CliExecution {
                exit_code: 0,
                stdout: format!(
                    "\nActive profile: {active}\nPath:           {}\nGateway:        stopped\nSkills:         {skills} installed\n\n",
                    profile_dir.display()
                ),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "create", name, "--no-alias", "--no-skills", "--description", description] =>
        {
            let dir = profile_dir(hermes_home, name);
            create_minimal_profile(&dir, Some(description), true)?;
            result = CliExecution {
                exit_code: 0,
                stdout: format!(
                    "\nProfile '{name}' created at {}\nNo bundled skills seeded (--no-skills). Delete .no-bundled-skills in the profile to opt back in.\n\nNext steps:\n  {name} setup              Configure API keys and model\n  {name} chat               Start chatting\n  {name} gateway start      Start the messaging gateway\n\n  ⚠ This profile has no API keys yet. Run '{name} setup' first,\n    or it will inherit keys from your shell environment.\n  Edit {}/SOUL.md to customize personality\n\n",
                    dir.display(),
                    dir.display(),
                ),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "describe", name] => {
            let dir = profile_dir(hermes_home, name);
            let description = read_profile_description(&dir)?.unwrap_or_default();
            let stdout = if description.is_empty() {
                format!("(no description set for '{name}')\n")
            } else {
                format!("{description}\n")
            };
            result = CliExecution {
                exit_code: 0,
                stdout,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "describe", name, "--text", description] => {
            let dir = profile_dir(hermes_home, name);
            write_profile_description(&dir, description)?;
            result = CliExecution {
                exit_code: 0,
                stdout: format!("Description updated for '{name}'.\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "show", name] => {
            let dir = profile_dir(hermes_home, name);
            let skills = count_profile_skills(&dir);
            result = CliExecution {
                exit_code: 0,
                stdout: format!(
                    "\nProfile: {name}\nPath:    {}\nGateway: stopped\nSkills:  {skills}\n.env:    {}\nSOUL.md: {}\n\n",
                    dir.display(),
                    if dir.join(".env").exists() { "exists" } else { "not configured" },
                    if dir.join("SOUL.md").exists() { "exists" } else { "not configured" },
                ),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "use", name] => {
            fs::create_dir_all(hermes_home)?;
            fs::write(hermes_home.join("active_profile"), format!("{name}\n"))?;
            result = CliExecution {
                exit_code: 0,
                stdout: format!("Switched to: {name}\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "list"] => {
            let active = active_profile_name(hermes_home);
            let mut stdout = "\n Profile          Model                        Gateway      Alias        Distribution\n ───────────────    ───────────────────────────    ───────────    ───────────    ────────────────────\n".to_string();
            stdout.push_str(&format!(
                "{}{:<15} {:<28} {:<12} {:<12} {}\n",
                if active == "default" { " ◆" } else { "  " },
                "default",
                "—",
                "stopped",
                "—",
                "—"
            ));
            let profiles_root = hermes_home.join("profiles");
            if profiles_root.is_dir() {
                let mut names = fs::read_dir(&profiles_root)?
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_dir())
                    .map(|entry| entry.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>();
                names.sort();
                for name in names {
                    stdout.push_str(&format!(
                        "{}{:<15} {:<28} {:<12} {:<12} {}\n",
                        if active == name { " ◆" } else { "  " },
                        name,
                        "—",
                        "stopped",
                        "—",
                        "—"
                    ));
                }
            }
            stdout.push('\n');
            result = CliExecution {
                exit_code: 0,
                stdout,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "delete", name, "--yes"]
        | ["hermes", "profile", "delete", name, "-y"] => {
            let dir = profile_dir(hermes_home, name);
            let _ = fs::remove_dir_all(&dir);
            let mut stdout = format!(
                "\nProfile: {name}\nPath:    {}\n\nThis will permanently delete:\n  • All config, API keys, memories, sessions, skills, cron jobs\n✓ Removed {}\n",
                dir.display(),
                dir.display(),
            );
            if active_profile_name(hermes_home) == *name {
                let _ = fs::remove_file(hermes_home.join("active_profile"));
                stdout.push_str("✓ Active profile reset to default\n");
            }
            stdout.push_str(&format!("\nProfile '{name}' deleted.\n"));
            result = CliExecution {
                exit_code: 0,
                stdout,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "profile", "rename", old_name, new_name] => {
            match rename_profile_dir(hermes_home, old_name, new_name) {
                Ok(new_dir) => {
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!(
                            "✓ Renamed {old_name} → {new_name}\n\nProfile renamed: {old_name} → {new_name}\nPath: {}\n\n",
                            new_dir.display()
                        ),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
                Err(error) => {
                    result = CliExecution {
                        exit_code: 1,
                        stdout: format!("Error: {error}\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
            }
        }
        ["hermes", "profile", "export", name, "-o", output]
        | ["hermes", "profile", "export", name, "--output", output] => {
            match export_profile_archive(hermes_home, name, Path::new(output)) {
                Ok(path) => {
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!("✓ Exported '{name}' to {}\n", path.display()),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
                Err(error) => {
                    result = CliExecution {
                        exit_code: 1,
                        stdout: format!("Error: {error}\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
            }
        }
        ["hermes", "profile", "import", archive, "--name", name] => {
            match import_profile_archive(hermes_home, Path::new(archive), Some(name)) {
                Ok(dir) => {
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!(
                            "✓ Imported profile '{}' at {}\n\n",
                            dir.file_name()
                                .and_then(|value| value.to_str())
                                .unwrap_or(name),
                            dir.display()
                        ),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
                Err(error) => {
                    result = CliExecution {
                        exit_code: 1,
                        stdout: format!("Error: {error}\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
            }
        }
        ["hermes", "profile", "import", archive] => {
            match import_profile_archive(hermes_home, Path::new(archive), None) {
                Ok(dir) => {
                    let name = dir
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("imported");
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!("✓ Imported profile '{name}' at {}\n\n", dir.display()),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
                Err(error) => {
                    result = CliExecution {
                        exit_code: 1,
                        stdout: format!("Error: {error}\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
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
        ["hermes", "sessions", "prune", "--older-than", days, "--source", source, "--yes"]
        | ["hermes", "sessions", "prune", "--older-than", days, "--source", source, "-y"] => {
            let db = open_session_db(hermes_home)?;
            let days = days.parse::<i64>().unwrap_or(90);
            let sessions_dir = hermes_home.join("sessions");
            let count = db
                .prune_sessions(days, Some(source), Some(&sessions_dir))
                .map_err(io::Error::other)?;
            result = CliExecution {
                exit_code: 0,
                stdout: format!("Pruned {count} session(s).\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "sessions", "prune", "--older-than", days, "--yes"]
        | ["hermes", "sessions", "prune", "--older-than", days, "-y"] => {
            let db = open_session_db(hermes_home)?;
            let days = days.parse::<i64>().unwrap_or(90);
            let sessions_dir = hermes_home.join("sessions");
            let count = db
                .prune_sessions(days, None, Some(&sessions_dir))
                .map_err(io::Error::other)?;
            result = CliExecution {
                exit_code: 0,
                stdout: format!("Pruned {count} session(s).\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        _ => {}
    }

    Ok(result)
}

fn default_cli_saved_toolsets() -> BTreeSet<String> {
    DEFAULT_CLI_SAVED_TOOLSETS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn default_display_enabled_toolsets() -> BTreeSet<String> {
    DISPLAY_TOOLSETS
        .iter()
        .filter_map(|(name, _, enabled)| enabled.then_some((*name).to_string()))
        .collect()
}

fn read_config_value(hermes_home: &Path) -> io::Result<Value> {
    let config_path = hermes_home.join("config.yaml");
    if config_path.exists() {
        Ok(
            serde_yaml::from_str::<Value>(&fs::read_to_string(config_path)?)
                .unwrap_or_else(|_| json!({})),
        )
    } else {
        Ok(json!({}))
    }
}

fn read_cli_toolsets(hermes_home: &Path) -> io::Result<BTreeSet<String>> {
    let config = read_config_value(hermes_home)?;
    Ok(cli_toolsets_from_config(&config).unwrap_or_else(default_display_enabled_toolsets))
}

fn cli_toolsets_from_config(config: &Value) -> Option<BTreeSet<String>> {
    config
        .get("platform_toolsets")
        .and_then(|value| value.get("cli"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
}

fn apply_cli_toolset_change(hermes_home: &Path, names: &[&str], enable: bool) -> io::Result<()> {
    fs::create_dir_all(hermes_home)?;
    let mut config = read_config_value(hermes_home)?;
    let mut enabled = cli_toolsets_from_config(&config).unwrap_or_else(default_cli_saved_toolsets);
    for name in names {
        if enable {
            enabled.insert((*name).to_string());
        } else {
            enabled.remove(*name);
        }
    }

    let Some(root) = config.as_object_mut() else {
        config = json!({});
        return apply_cli_toolset_change_with_config(hermes_home, config, enabled);
    };
    let platform_toolsets = root
        .entry("platform_toolsets".to_string())
        .or_insert_with(|| json!({}));
    if !platform_toolsets.is_object() {
        *platform_toolsets = json!({});
    }
    platform_toolsets.as_object_mut().unwrap().insert(
        "cli".to_string(),
        Value::Array(enabled.into_iter().map(Value::String).collect()),
    );
    fs::write(
        hermes_home.join("config.yaml"),
        serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string()),
    )
}

fn apply_cli_toolset_change_with_config(
    hermes_home: &Path,
    mut config: Value,
    enabled: BTreeSet<String>,
) -> io::Result<()> {
    let root = config.as_object_mut().unwrap();
    root.insert(
        "platform_toolsets".to_string(),
        json!({"cli": enabled.into_iter().collect::<Vec<_>>()}),
    );
    fs::write(
        hermes_home.join("config.yaml"),
        serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string()),
    )
}

fn tools_list_output(enabled: &BTreeSet<String>) -> String {
    let mut stdout = "Built-in toolsets (cli):\n".to_string();
    for (name, label, _) in DISPLAY_TOOLSETS {
        let status = if enabled.contains(*name) {
            "✓ enabled"
        } else {
            "✗ disabled"
        };
        stdout.push_str(&format!("  {status}  {name}  {label}\n"));
    }
    stdout
}

fn cron_jobs_path(hermes_home: &Path) -> PathBuf {
    hermes_home.join("cron").join("jobs.json")
}

fn create_cron_job(
    hermes_home: &Path,
    schedule_text: &str,
    prompt: &str,
    name: &str,
    deliver: &str,
) -> io::Result<()> {
    let path = cron_jobs_path(hermes_home);
    let mut jobs = hermes_cron::load_jobs(&path)?;
    let schedule = hermes_cron::parse_schedule(schedule_text).map_err(io::Error::other)?;
    let display = schedule
        .get("display")
        .and_then(Value::as_str)
        .unwrap_or(schedule_text)
        .to_string();
    let next_run_at = schedule
        .get("run_at")
        .and_then(Value::as_str)
        .unwrap_or(schedule_text)
        .to_string();
    jobs.push(json!({
        "id": format!("rust-{name}"),
        "name": name,
        "prompt": prompt,
        "skills": [],
        "skill": null,
        "schedule": schedule,
        "schedule_display": display,
        "repeat": {"times": 1, "completed": 0},
        "enabled": true,
        "state": "scheduled",
        "next_run_at": next_run_at,
        "deliver": deliver,
    }));
    save_cron_jobs_object(&path, &jobs)
}

fn set_cron_job_enabled(hermes_home: &Path, name: &str, enabled: bool) -> io::Result<()> {
    let path = cron_jobs_path(hermes_home);
    let mut jobs = hermes_cron::load_jobs(&path)?;
    for job in &mut jobs {
        if cron_job_matches(job, name) {
            if let Some(object) = job.as_object_mut() {
                object.insert("enabled".to_string(), json!(enabled));
                object.insert(
                    "state".to_string(),
                    json!(if enabled { "scheduled" } else { "paused" }),
                );
            }
        }
    }
    save_cron_jobs_object(&path, &jobs)
}

fn remove_cron_job(hermes_home: &Path, name: &str) -> io::Result<()> {
    let path = cron_jobs_path(hermes_home);
    let mut jobs = hermes_cron::load_jobs(&path)?;
    jobs.retain(|job| !cron_job_matches(job, name));
    save_cron_jobs_object(&path, &jobs)
}

fn cron_job_matches(job: &Value, name_or_id: &str) -> bool {
    job.get("id").and_then(Value::as_str) == Some(name_or_id)
        || job.get("name").and_then(Value::as_str) == Some(name_or_id)
}

fn save_cron_jobs_object(path: &Path, jobs: &[Value]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let normalized = jobs
        .iter()
        .map(hermes_cron::normalize_job_record)
        .collect::<Vec<_>>();
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({"jobs": normalized, "updated_at": "<timestamp>"}))
            .map_err(io::Error::other)?,
    )
}

fn cron_list_output(jobs: &[Value]) -> String {
    if jobs.is_empty() {
        return "No scheduled jobs.\nCreate one with 'hermes cron create ...' or the /cron command in chat.\n".to_string();
    }
    let mut stdout = "\n┌─────────────────────────────────────────────────────────────────────────┐\n│                         Scheduled Jobs                                  │\n└─────────────────────────────────────────────────────────────────────────┘\n\n".to_string();
    for job in jobs {
        let id = job.get("id").and_then(Value::as_str).unwrap_or("unknown");
        let name = job.get("name").and_then(Value::as_str).unwrap_or(id);
        let state = job
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("scheduled");
        let display = job
            .get("schedule_display")
            .and_then(Value::as_str)
            .unwrap_or("?");
        let next_run = job.get("next_run_at").and_then(Value::as_str).unwrap_or("");
        let deliver = job
            .get("deliver")
            .and_then(Value::as_str)
            .unwrap_or("local");
        stdout.push_str(&format!(
            "  {id} [{}]\n    Name:      {name}\n    Schedule:  {display}\n    Repeat:    0/1\n    Next run:  {next_run}\n    Deliver:   {deliver}\n\n",
            if state == "paused" { "paused" } else { "active" }
        ));
    }
    stdout.push_str("  ⚠  Gateway is not running — jobs won't fire automatically.\n     Start it with: hermes gateway install\n                    sudo hermes gateway install --system  # Linux servers\n\n");
    stdout
}

fn open_session_db(hermes_home: &Path) -> io::Result<hermes_session::SqliteSessionStore> {
    hermes_session::SqliteSessionStore::open(hermes_home.join("state.db")).map_err(io::Error::other)
}

fn profile_dir(hermes_home: &Path, name: &str) -> PathBuf {
    if name == "default" {
        hermes_home.to_path_buf()
    } else {
        hermes_home.join("profiles").join(name)
    }
}

fn active_profile_name(hermes_home: &Path) -> String {
    fs::read_to_string(hermes_home.join("active_profile"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

fn create_minimal_profile(
    profile_dir: &Path,
    description: Option<&str>,
    no_skills: bool,
) -> io::Result<()> {
    for dir in [
        "memories",
        "sessions",
        "skills",
        "skins",
        "logs",
        "plans",
        "workspace",
        "cron",
        "home",
    ] {
        fs::create_dir_all(profile_dir.join(dir))?;
    }
    let soul = profile_dir.join("SOUL.md");
    if !soul.exists() {
        fs::write(soul, "# Hermes Soul\n")?;
    }
    if no_skills {
        fs::write(
            profile_dir.join(".no-bundled-skills"),
            "This profile opted out of bundled-skill seeding (`hermes profile create --no-skills`).\n",
        )?;
    }
    if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
        write_profile_description(profile_dir, description)?;
    }
    Ok(())
}

fn write_profile_description(profile_dir: &Path, description: &str) -> io::Result<()> {
    fs::create_dir_all(profile_dir)?;
    let meta = json!({
        "description": description.trim(),
        "description_auto": false,
    });
    let yaml = serde_yaml::to_string(&meta).unwrap_or_else(|_| "{}\n".to_string());
    fs::write(profile_dir.join("profile.yaml"), yaml)
}

fn read_profile_description(profile_dir: &Path) -> io::Result<Option<String>> {
    let path = profile_dir.join("profile.yaml");
    if !path.exists() {
        return Ok(None);
    }
    let data =
        serde_yaml::from_str::<Value>(&fs::read_to_string(path)?).unwrap_or_else(|_| json!({}));
    Ok(data
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string))
}

fn count_profile_skills(profile_dir: &Path) -> usize {
    let skills_dir = profile_dir.join("skills");
    count_skill_files(&skills_dir).unwrap_or(0)
}

fn rename_profile_dir(hermes_home: &Path, old_name: &str, new_name: &str) -> io::Result<PathBuf> {
    let old_canon = normalize_profile_name_for_cli(old_name)?;
    let new_canon = normalize_profile_name_for_cli(new_name)?;
    validate_profile_name_for_cli(&old_canon)?;
    validate_profile_name_for_cli(&new_canon)?;
    if old_canon == "default" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cannot rename the default profile.",
        ));
    }
    if new_canon == "default" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cannot rename to 'default' — it is reserved.",
        ));
    }
    let old_dir = profile_dir(hermes_home, &old_canon);
    let new_dir = profile_dir(hermes_home, &new_canon);
    if !old_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Profile '{old_canon}' does not exist."),
        ));
    }
    if new_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Profile '{new_canon}' already exists."),
        ));
    }
    fs::rename(&old_dir, &new_dir)?;
    if active_profile_name(hermes_home) == old_canon {
        fs::write(hermes_home.join("active_profile"), format!("{new_canon}\n"))?;
    }
    Ok(new_dir)
}

fn export_profile_archive(hermes_home: &Path, name: &str, output: &Path) -> io::Result<PathBuf> {
    let dir = profile_dir(hermes_home, name);
    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Profile '{name}' does not exist."),
        ));
    }

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(output)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);
    let archive_root = if name == "default" { "default" } else { name };
    builder.append_dir(archive_root, &dir)?;
    append_profile_archive_entries(
        &mut builder,
        &dir,
        Path::new(archive_root),
        name == "default",
        true,
    )?;
    builder.finish()?;
    Ok(output.to_path_buf())
}

fn append_profile_archive_entries(
    builder: &mut Builder<GzEncoder<fs::File>>,
    source: &Path,
    archive_prefix: &Path,
    is_default_profile: bool,
    at_root: bool,
) -> io::Result<()> {
    let mut entries = fs::read_dir(source)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_profile_export_entry(&name, is_default_profile, at_root) {
            continue;
        }
        let path = entry.path();
        let archive_path = archive_prefix.join(&name);
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            builder.append_dir(&archive_path, &path)?;
            append_profile_archive_entries(
                builder,
                &path,
                &archive_path,
                is_default_profile,
                false,
            )?;
        } else if metadata.is_file() {
            builder.append_path_with_name(&path, &archive_path)?;
        }
    }
    Ok(())
}

fn should_skip_profile_export_entry(name: &str, is_default_profile: bool, at_root: bool) -> bool {
    if !is_default_profile {
        return matches!(name, ".env" | "auth.json");
    }
    if hermes_config::export_ignore(&[name], at_root)
        .as_array()
        .is_some_and(|values| !values.is_empty())
    {
        return true;
    }
    false
}

fn import_profile_archive(
    hermes_home: &Path,
    archive: &Path,
    name: Option<&str>,
) -> io::Result<PathBuf> {
    if !archive.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Archive not found: {}", archive.display()),
        ));
    }

    let roots = inspect_profile_archive_roots(archive)?;
    let archive_root = if roots.len() == 1 {
        roots[0].clone()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Profile archive must contain exactly one top-level directory.",
        ));
    };
    let inferred = name.unwrap_or(&archive_root);
    let canon = normalize_profile_name_for_cli(inferred)?;
    validate_profile_name_for_cli(&canon)?;
    if canon == "default" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cannot import as 'default' — that is the built-in root profile (~/.hermes). Specify a different name: hermes profile import <archive> --name <name>",
        ));
    }
    let target = profile_dir(hermes_home, &canon);
    if target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Profile '{canon}' already exists at {}", target.display()),
        ));
    }
    fs::create_dir_all(
        target
            .parent()
            .ok_or_else(|| io::Error::other("profile target has no parent"))?,
    )?;

    let file = fs::File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut archive_reader = Archive::new(decoder);
    for entry in archive_reader.entries()? {
        let mut entry = entry?;
        let parts = normalize_archive_member_parts(&entry.path()?.to_string_lossy())?;
        if parts.first() != Some(&archive_root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Profile archive must contain exactly one top-level directory.",
            ));
        }
        let rel = parts.iter().skip(1).collect::<PathBuf>();
        let destination = target.join(rel);
        let entry_type = entry.header().entry_type();
        if entry_type == EntryType::Directory {
            fs::create_dir_all(&destination)?;
        } else if entry_type == EntryType::Regular {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(&destination)?;
            io::copy(&mut entry, &mut output)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported archive member type: {}", parts.join("/")),
            ));
        }
    }
    Ok(target)
}

pub fn profile_archive_members(path: &Path) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let mut names = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        names.push(entry.path()?.to_string_lossy().replace('\\', "/"));
    }
    names.sort();
    Ok(names)
}

fn inspect_profile_archive_roots(path: &Path) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let mut roots = BTreeSet::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let parts = normalize_archive_member_parts(&entry.path()?.to_string_lossy())?;
        if let Some(root) = parts.first() {
            roots.insert(root.clone());
        }
    }
    Ok(roots.into_iter().collect())
}

fn normalize_archive_member_parts(member_name: &str) -> io::Result<Vec<String>> {
    let normalized = member_name.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized
            .as_bytes()
            .get(1)
            .is_some_and(|value| *value == b':')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsafe archive member path: {member_name}"),
        ));
    }
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsafe archive member path: {member_name}"),
            ));
        }
        parts.push(part.to_string());
    }
    if parts.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsafe archive member path: {member_name}"),
        ));
    }
    Ok(parts)
}

fn normalize_profile_name_for_cli(name: &str) -> io::Result<String> {
    let stripped = name.trim();
    if stripped.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name cannot be empty",
        ));
    }
    if stripped.eq_ignore_ascii_case("default") {
        return Ok("default".to_string());
    }
    Ok(stripped.to_ascii_lowercase())
}

fn validate_profile_name_for_cli(name: &str) -> io::Result<()> {
    if name == "default" {
        return Ok(());
    }
    let bytes = name.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        });
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid profile name '{name}'. Must match [a-z0-9][a-z0-9_-]{{0,63}}"),
        ));
    }
    if matches!(name, "hermes" | "test" | "tmp" | "root" | "sudo") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Profile name '{name}' is reserved — it collides with either the Hermes installation itself or a common system binary.  Pick a different name."),
        ));
    }
    Ok(())
}

fn count_skill_files(dir: &Path) -> io::Result<usize> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count += count_skill_files(&path)?;
        } else if path.file_name().is_some_and(|name| name == "SKILL.md") {
            count += 1;
        }
    }
    Ok(count)
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

pub fn tui_gateway_method_names() -> &'static [&'static str] {
    TUI_GATEWAY_METHODS
}

pub fn tui_gateway_long_handlers() -> &'static [&'static str] {
    TUI_GATEWAY_LONG_HANDLERS
}

pub fn tui_jsonrpc_ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub fn tui_jsonrpc_err(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

pub fn tui_unknown_method_response(id: Value, method: &str) -> Value {
    tui_jsonrpc_err(id, -32601, &format!("unknown method: {method}"))
}

pub fn tui_normalize_request(req: &Value) -> Value {
    let Some(obj) = req.as_object() else {
        return json!({
            "ok": false,
            "response": tui_jsonrpc_err(Value::Null, -32600, "invalid request: expected an object"),
        });
    };

    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let method = obj.get("method").and_then(Value::as_str).unwrap_or("");
    if method.is_empty() {
        return json!({
            "ok": false,
            "response": tui_jsonrpc_err(id, -32600, "invalid request: method must be a non-empty string"),
        });
    }

    let params = match obj.get("params") {
        None | Some(Value::Null) => json!({}),
        Some(Value::Object(_)) => obj["params"].clone(),
        Some(_) => {
            return json!({
                "ok": false,
                "response": tui_jsonrpc_err(id, -32602, "invalid params: expected an object"),
            });
        }
    };

    json!({"ok": true, "id": id, "method": method, "params": params})
}

pub fn tui_event_frame(event: &str, session_id: &str, payload: Option<Value>) -> Value {
    let mut params = serde_json::Map::new();
    params.insert("type".to_string(), json!(event));
    params.insert("session_id".to_string(), json!(session_id));
    if let Some(payload) = payload {
        params.insert("payload".to_string(), payload);
    }
    json!({"jsonrpc": "2.0", "method": "event", "params": Value::Object(params)})
}

pub fn parse_dashboard_pty_resize_frame(raw: &[u8]) -> Option<(usize, usize)> {
    const PREFIX: &[u8] = b"\x1b[RESIZE:";
    if !raw.starts_with(PREFIX) || !raw.ends_with(b"]") {
        return None;
    }
    let body = &raw[PREFIX.len()..raw.len() - 1];
    let (cols, rows) = split_once_byte(body, b';')?;
    if cols.is_empty() || rows.is_empty() {
        return None;
    }
    let cols = parse_ascii_usize(cols)?;
    let rows = parse_ascii_usize(rows)?;
    Some((cols, rows))
}

pub fn valid_dashboard_event_channel(channel: &str) -> bool {
    !channel.is_empty()
        && channel.len() <= 128
        && channel
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn dashboard_channel_or_none(channel: &str) -> Option<String> {
    valid_dashboard_event_channel(channel).then(|| channel.to_string())
}

pub fn dashboard_build_sidecar_url(host: Option<&str>, port: Option<u16>, channel: &str) -> Value {
    let (Some(host), Some(port)) = (host, port) else {
        return Value::Null;
    };
    let netloc = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    json!(format!(
        "ws://{netloc}/api/pub?token=<token>&channel={}",
        url_query_escape(channel)
    ))
}

pub fn dashboard_ws_client_allowed(bound_host: &str, client_host: Option<&str>) -> bool {
    if matches!(bound_host, "0.0.0.0" | "::") {
        return true;
    }
    let Some(client_host) = client_host else {
        return true;
    };
    if client_host.is_empty() {
        return true;
    }
    matches!(
        client_host,
        "127.0.0.1" | "::1" | "localhost" | "testclient"
    )
}

pub fn dashboard_normalise_prefix(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return String::new();
    };
    let mut prefix = raw.trim().to_string();
    if prefix.is_empty() {
        return String::new();
    }
    if !prefix.starts_with('/') {
        prefix.insert(0, '/');
    }
    while prefix.ends_with('/') {
        prefix.pop();
    }
    if prefix.contains("//")
        || prefix.contains("..")
        || prefix
            .chars()
            .any(|ch| matches!(ch, '"' | '\'' | '<' | '>' | ' ' | '\n' | '\r' | '\t'))
        || prefix.len() > 64
    {
        return String::new();
    }
    prefix
}

pub fn tui_cli_exec_blocked(argv: &[&str]) -> Option<&'static str> {
    if argv.is_empty() {
        return Some("bare `hermes` is interactive — use `/hermes chat -q …` or run `hermes` in another terminal");
    }
    let first = argv[0].to_ascii_lowercase();
    match first.as_str() {
        "setup" => Some("`hermes setup` needs a full terminal — run it outside the TUI"),
        "gateway" => Some("`hermes gateway` is long-running — run it in another terminal"),
        "sessions" if argv.get(1).is_some_and(|value| value.eq_ignore_ascii_case("browse")) => {
            Some("`hermes sessions browse` is interactive — use /resume here, or run browse in another terminal")
        }
        "config" if argv.get(1).is_some_and(|value| value.eq_ignore_ascii_case("edit")) => {
            Some("`hermes config edit` needs $EDITOR in a real terminal")
        }
        _ => None,
    }
}

pub fn tui_details_completions(text: &str) -> Option<Vec<Value>> {
    if !text.to_ascii_lowercase().starts_with("/details") {
        return None;
    }

    let stripped = text.trim();
    if !stripped.is_empty()
        && !"/details".starts_with(
            stripped
                .to_ascii_lowercase()
                .split_whitespace()
                .next()
                .unwrap_or(""),
        )
    {
        return None;
    }

    let mut body = text.get("/details".len()..).unwrap_or("");
    if let Some(rest) = body.strip_prefix(' ') {
        body = rest;
    }
    let parts = body.split_whitespace().collect::<Vec<_>>();
    let has_trailing_space = text.ends_with(' ');

    if body.is_empty() || (parts.is_empty() && has_trailing_space) {
        let needs_leading_space = !has_trailing_space;
        let mut items = Vec::new();
        for mode in DETAIL_MODES {
            items.push(details_root_completion_item(
                mode,
                "global mode",
                needs_leading_space,
            ));
        }
        items.push(details_root_completion_item(
            "cycle",
            "cycle global mode",
            needs_leading_space,
        ));
        for section in DETAIL_SECTION_NAMES {
            items.push(details_root_completion_item(
                section,
                "section override",
                needs_leading_space,
            ));
        }
        return Some(items);
    }

    if parts.len() == 1 && !has_trailing_space {
        let prefix = parts[0].to_ascii_lowercase();
        let candidates = DETAIL_MODES
            .iter()
            .copied()
            .chain(["cycle"])
            .chain(DETAIL_SECTION_NAMES.iter().copied());
        return Some(
            candidates
                .filter(|candidate| candidate.starts_with(&prefix) && *candidate != prefix)
                .map(|candidate| {
                    let meta = if DETAIL_SECTION_NAMES.contains(&candidate) {
                        "section override"
                    } else if candidate == "cycle" {
                        "cycle global mode"
                    } else {
                        "global mode"
                    };
                    details_completion_item(candidate, meta)
                })
                .collect(),
        );
    }

    if parts.len() == 1 && has_trailing_space && DETAIL_SECTION_NAMES.contains(&parts[0]) {
        let section = parts[0].to_ascii_lowercase();
        let mut items = DETAIL_MODES
            .iter()
            .map(|mode| details_completion_item(mode, &format!("set {section}")))
            .collect::<Vec<_>>();
        items.push(details_completion_item(
            "reset",
            &format!("clear {section} override"),
        ));
        return Some(items);
    }

    if parts.len() == 2 && !has_trailing_space && DETAIL_SECTION_NAMES.contains(&parts[0]) {
        let section = parts[0].to_ascii_lowercase();
        let prefix = parts[1].to_ascii_lowercase();
        return Some(
            DETAIL_MODES
                .iter()
                .copied()
                .chain(["reset"])
                .filter(|candidate| candidate.starts_with(&prefix) && *candidate != prefix)
                .map(|candidate| {
                    let meta = if candidate == "reset" {
                        format!("clear {section} override")
                    } else {
                        format!("set {section}")
                    };
                    details_completion_item(candidate, &meta)
                })
                .collect(),
        );
    }

    Some(Vec::new())
}

pub fn tui_complete_slash_details_response(id: Value, text: &str) -> Option<Value> {
    let items = tui_details_completions(text)?;
    let replace_from = text.rfind(' ').map(|index| index + 1).unwrap_or(text.len());
    Some(tui_jsonrpc_ok(
        id,
        json!({"items": items, "replace_from": replace_from}),
    ))
}

pub fn tui_session_not_found(id: Value) -> Value {
    tui_jsonrpc_err(id, 4001, "session not found")
}

pub fn tui_terminal_resize_response(id: Value, cols: usize) -> Value {
    tui_jsonrpc_ok(id, json!({"cols": cols}))
}

pub fn tui_empty_session_usage_response(id: Value) -> Value {
    tui_jsonrpc_ok(id, json!({"calls": 0, "input": 0, "output": 0, "total": 0}))
}

pub fn tui_session_history_response(id: Value, history: &[Value]) -> Value {
    let messages = tui_history_to_messages(history);
    tui_jsonrpc_ok(id, json!({"count": history.len(), "messages": messages}))
}

pub fn tui_steer_empty_response(id: Value) -> Value {
    tui_jsonrpc_err(id, 4002, "text is required")
}

pub fn tui_steer_no_agent_response(id: Value) -> Value {
    tui_jsonrpc_err(id, 4010, "agent does not support steer")
}

pub fn tui_prompt_busy_response(id: Value) -> Value {
    tui_jsonrpc_err(id, 4009, "session busy")
}

pub fn tui_history_to_messages(history: &[Value]) -> Vec<Value> {
    let mut messages = Vec::new();
    for message in history {
        let Some(obj) = message.as_object() else {
            continue;
        };
        let role = obj.get("role").and_then(Value::as_str).unwrap_or("");
        if !matches!(role, "user" | "assistant" | "tool" | "system") {
            continue;
        }

        let content_text = tui_content_display_text(obj.get("content").unwrap_or(&Value::Null));
        if role == "assistant" && obj.get("tool_calls").is_some() && content_text.trim().is_empty()
        {
            continue;
        }
        if role == "tool" {
            let name = obj
                .get("tool_name")
                .and_then(Value::as_str)
                .unwrap_or("tool");
            messages.push(json!({"role": "tool", "name": name, "context": ""}));
            continue;
        }
        if content_text.trim().is_empty() {
            continue;
        }
        messages.push(json!({"role": role, "text": content_text}));
    }
    messages
}

pub fn tui_content_display_text(content: &Value) -> String {
    match content {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Array(parts) => parts
            .iter()
            .map(tui_content_display_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(obj) => {
            if let Some(kind) = obj.get("type").and_then(Value::as_str) {
                match kind {
                    "text" | "input_text" | "output_text" => obj
                        .get("text")
                        .or_else(|| obj.get("content"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "image_url" | "input_image" | "image" => "[image]".to_string(),
                    "input_audio" | "audio" => "[audio]".to_string(),
                    other => format!("[{other}]"),
                }
            } else if let Some(text) = obj.get("text").and_then(Value::as_str) {
                text.to_string()
            } else {
                "[structured content]".to_string()
            }
        }
        other => other.to_string(),
    }
}

pub fn plugin_valid_hooks() -> &'static [&'static str] {
    PLUGIN_VALID_HOOKS
}

pub fn plugin_valid_kinds() -> &'static [&'static str] {
    PLUGIN_VALID_KINDS
}

pub fn plugin_entry_points_group() -> &'static str {
    "hermes_agent.plugins"
}

pub fn scan_plugin_manifests(
    root: &Path,
    source: &str,
    skip_names: &[&str],
) -> io::Result<Vec<Value>> {
    let skip_names = skip_names.iter().copied().collect::<BTreeSet<_>>();
    scan_plugin_manifest_level(root, source, &skip_names, "", 0)
}

pub fn plugin_load_policy(
    bundled_root: &Path,
    user_root: &Path,
    enabled: &[&str],
    disabled: &[&str],
) -> io::Result<Value> {
    plugin_load_policy_inner(bundled_root, user_root, None, enabled, disabled)
}

pub fn plugin_load_policy_with_project(
    bundled_root: &Path,
    user_root: &Path,
    project_root: &Path,
    enabled: &[&str],
    disabled: &[&str],
) -> io::Result<Value> {
    plugin_load_policy_inner(
        bundled_root,
        user_root,
        Some(project_root),
        enabled,
        disabled,
    )
}

pub fn memory_provider_discovery(
    bundled_root: &Path,
    user_root: &Path,
    names_to_find: &[&str],
    heuristic_names: &[&str],
) -> io::Result<Value> {
    let provider_dirs = memory_provider_dirs(bundled_root, user_root)?;
    let mut find_provider_dir = serde_json::Map::new();
    for name in names_to_find {
        let found = find_memory_provider_dir(bundled_root, user_root, name)?;
        find_provider_dir.insert(
            (*name).to_string(),
            found.map_or(Value::Null, |path| json!(path.to_string_lossy())),
        );
    }

    let mut heuristics = serde_json::Map::new();
    for name in heuristic_names {
        heuristics.insert(
            (*name).to_string(),
            json!(is_memory_provider_dir(&user_root.join(name))),
        );
    }

    Ok(json!({
        "provider_dirs": provider_dirs
            .into_iter()
            .map(|(name, path)| json!({"name": name, "path": path.to_string_lossy()}))
            .collect::<Vec<_>>(),
        "find_provider_dir": Value::Object(find_provider_dir),
        "heuristics": Value::Object(heuristics),
    }))
}

pub fn provider_registry_selection(section: &Value, kind: &str) -> Value {
    let providers = section
        .get("providers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let get_provider_inputs = section
        .get("get_provider_inputs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut get_provider = serde_json::Map::new();
    for (name, input) in get_provider_inputs {
        get_provider.insert(name, registry_get_provider(&providers, &input));
    }

    let resolution_cases = section
        .get("resolution_cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .map(|case| {
                    let providers = case
                        .get("providers")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let configured = case.get("configured").and_then(Value::as_str);
                    let active = match kind {
                        "image_gen" => image_gen_active_provider(&providers, configured),
                        "web" => web_active_provider(
                            &providers,
                            configured,
                            case.get("capability")
                                .and_then(Value::as_str)
                                .unwrap_or("search"),
                        ),
                        "browser" => browser_active_provider(&providers, configured),
                        _ => None,
                    };
                    let mut output = case.clone();
                    output["active"] = active.map_or(Value::Null, Value::String);
                    output
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut output = serde_json::Map::new();
    output.insert("list".to_string(), json!(registry_sorted_names(&providers)));
    output.insert("get_provider".to_string(), Value::Object(get_provider));
    if kind == "web" {
        output.insert(
            "legacy_preference".to_string(),
            json!(WEB_LEGACY_PREFERENCE),
        );
    } else if kind == "browser" {
        output.insert(
            "legacy_preference".to_string(),
            json!(BROWSER_LEGACY_PREFERENCE),
        );
    }
    output.insert(
        "resolution_cases".to_string(),
        Value::Array(resolution_cases),
    );
    Value::Object(output)
}

const WEB_LEGACY_PREFERENCE: &[&str] = &[
    "firecrawl",
    "parallel",
    "tavily",
    "exa",
    "searxng",
    "brave-free",
    "ddgs",
];

const BROWSER_LEGACY_PREFERENCE: &[&str] = &["browser-use", "browserbase"];

fn registry_sorted_names(providers: &[Value]) -> Vec<String> {
    let mut names = providers
        .iter()
        .filter_map(|provider| provider.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn registry_get_provider(providers: &[Value], input: &Value) -> Value {
    let Some(input) = input.as_str() else {
        return Value::Null;
    };
    let normalized = input.trim();
    providers
        .iter()
        .find_map(|provider| {
            let name = provider.get("name").and_then(Value::as_str)?;
            (name == normalized).then(|| Value::String(name.to_string()))
        })
        .unwrap_or(Value::Null)
}

fn provider_available(provider: &Value) -> bool {
    provider
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn image_gen_active_provider(providers: &[Value], configured: Option<&str>) -> Option<String> {
    if let Some(configured) = configured.filter(|value| !value.trim().is_empty()) {
        if let Some(provider) = provider_by_name(providers, configured) {
            return provider_name(provider).map(str::to_string);
        }
    }

    let available = providers
        .iter()
        .filter(|provider| provider_available(provider))
        .collect::<Vec<_>>();
    if available.len() == 1 {
        return provider_name(available[0]).map(str::to_string);
    }

    let fal = provider_by_name(providers, "fal")?;
    provider_available(fal)
        .then(|| provider_name(fal).map(str::to_string))
        .flatten()
}

fn web_active_provider(
    providers: &[Value],
    configured: Option<&str>,
    capability: &str,
) -> Option<String> {
    if let Some(configured) = configured.filter(|value| !value.trim().is_empty()) {
        if let Some(provider) = provider_by_name(providers, configured) {
            if web_provider_capable(provider, capability) {
                return provider_name(provider).map(str::to_string);
            }
        }
    }

    let eligible = providers
        .iter()
        .filter(|provider| {
            web_provider_capable(provider, capability) && provider_available(provider)
        })
        .collect::<Vec<_>>();
    if eligible.len() == 1 {
        return provider_name(eligible[0]).map(str::to_string);
    }

    for legacy in WEB_LEGACY_PREFERENCE {
        if let Some(provider) = provider_by_name(providers, legacy) {
            if web_provider_capable(provider, capability) && provider_available(provider) {
                return provider_name(provider).map(str::to_string);
            }
        }
    }

    None
}

fn browser_active_provider(providers: &[Value], configured: Option<&str>) -> Option<String> {
    if configured == Some("local") {
        return None;
    }

    if let Some(configured) = configured.filter(|value| !value.trim().is_empty()) {
        if let Some(provider) = provider_by_name(providers, configured) {
            return provider_name(provider).map(str::to_string);
        }
    }

    for legacy in BROWSER_LEGACY_PREFERENCE {
        if let Some(provider) = provider_by_name(providers, legacy) {
            if provider_available(provider) {
                return provider_name(provider).map(str::to_string);
            }
        }
    }

    None
}

fn provider_by_name<'a>(providers: &'a [Value], name: &str) -> Option<&'a Value> {
    providers
        .iter()
        .find(|provider| provider.get("name").and_then(Value::as_str) == Some(name))
}

fn provider_name(provider: &Value) -> Option<&str> {
    provider.get("name").and_then(Value::as_str)
}

fn web_provider_capable(provider: &Value, capability: &str) -> bool {
    match capability {
        "search" => provider
            .get("search")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "extract" => provider
            .get("extract")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "crawl" => provider
            .get("crawl")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

fn memory_provider_dirs(
    bundled_root: &Path,
    user_root: &Path,
) -> io::Result<Vec<(String, PathBuf)>> {
    let mut seen = BTreeSet::new();
    let mut dirs = Vec::new();

    if bundled_root.is_dir() {
        let mut children = fs::read_dir(bundled_root)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .collect::<Vec<_>>();
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let name = child.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if !child.path().join("__init__.py").exists() {
                continue;
            }
            seen.insert(name.clone());
            dirs.push((name, child.path()));
        }
    }

    if user_root.is_dir() {
        let mut children = fs::read_dir(user_root)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .collect::<Vec<_>>();
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let name = child.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') || seen.contains(&name) {
                continue;
            }
            if !is_memory_provider_dir(&child.path()) {
                continue;
            }
            dirs.push((name, child.path()));
        }
    }

    Ok(dirs)
}

fn find_memory_provider_dir(
    bundled_root: &Path,
    user_root: &Path,
    name: &str,
) -> io::Result<Option<PathBuf>> {
    let bundled = bundled_root.join(name);
    if bundled.is_dir() && bundled.join("__init__.py").exists() {
        return Ok(Some(bundled));
    }
    let user = user_root.join(name);
    if user.is_dir() && is_memory_provider_dir(&user) {
        return Ok(Some(user));
    }
    Ok(None)
}

fn is_memory_provider_dir(path: &Path) -> bool {
    let init_file = path.join("__init__.py");
    let Ok(source) = fs::read_to_string(init_file) else {
        return false;
    };
    let source = truncate_chars(&source, 8192);
    source.contains("register_memory_provider") || source.contains("MemoryProvider")
}

fn plugin_load_policy_inner(
    bundled_root: &Path,
    user_root: &Path,
    project_root: Option<&Path>,
    enabled: &[&str],
    disabled: &[&str],
) -> io::Result<Value> {
    let mut manifests = Vec::new();
    manifests.extend(scan_plugin_manifests(
        bundled_root,
        "bundled",
        &["memory", "context_engine", "platforms", "model-providers"],
    )?);
    manifests.extend(scan_plugin_manifests(
        &bundled_root.join("platforms"),
        "bundled",
        &[],
    )?);
    manifests.extend(scan_plugin_manifests(user_root, "user", &[])?);
    if let Some(project_root) = project_root {
        manifests.extend(scan_plugin_manifests(project_root, "project", &[])?);
    }

    let mut winners = BTreeMap::<String, Value>::new();
    for manifest in manifests {
        let key = manifest["key"].as_str().unwrap_or("").to_string();
        winners.insert(key, manifest);
    }

    let enabled = enabled.iter().copied().collect::<BTreeSet<_>>();
    let disabled = disabled.iter().copied().collect::<BTreeSet<_>>();
    let mut plugins = Vec::new();
    let mut registered_hooks = BTreeSet::new();
    let mut registered_commands = BTreeSet::new();

    for (key, manifest) in winners {
        let name = manifest["name"].as_str().unwrap_or("");
        let kind = manifest["kind"].as_str().unwrap_or("");
        let source = manifest["source"].as_str().unwrap_or("");
        let mut enabled_plugin = false;
        let mut error = Value::Null;
        let mut hooks_registered = Vec::<String>::new();
        let mut commands_registered = Vec::<String>::new();

        if disabled.contains(key.as_str()) || disabled.contains(name) {
            error = json!("disabled via config");
        } else if kind == "exclusive" {
            error = json!("exclusive plugin — activate via <category>.provider config");
        } else {
            let auto_enabled = kind == "model-provider"
                || (source == "bundled" && matches!(kind, "backend" | "platform"));
            let explicitly_enabled = enabled.contains(key.as_str()) || enabled.contains(name);
            if auto_enabled || explicitly_enabled {
                enabled_plugin = true;
            } else {
                error = json!(format!(
                    "not enabled in config (run `hermes plugins enable {key}` to activate)"
                ));
            }
        }

        if enabled_plugin {
            let path = Path::new(manifest["path"].as_str().unwrap_or(""));
            let init_text = fs::read_to_string(path.join("__init__.py")).unwrap_or_default();
            hooks_registered = extract_single_quoted_calls(&init_text, "register_hook");
            commands_registered = extract_single_quoted_calls(&init_text, "register_command");
            for hook in &hooks_registered {
                registered_hooks.insert(hook.clone());
            }
            for command in &commands_registered {
                registered_commands.insert(command.clone());
            }
        }

        plugins.push(json!({
            "key": key,
            "name": name,
            "kind": kind,
            "source": source,
            "enabled": enabled_plugin,
            "error": error,
            "hooks_registered": hooks_registered,
            "commands_registered": commands_registered,
        }));
    }

    Ok(json!({
        "plugins": plugins,
        "registered_hooks": registered_hooks.into_iter().collect::<Vec<_>>(),
        "registered_commands": registered_commands.into_iter().collect::<Vec<_>>(),
    }))
}

fn extract_single_quoted_calls(source: &str, function: &str) -> Vec<String> {
    let needle = format!("{function}('");
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find(&needle) {
        let after = &rest[index + needle.len()..];
        let Some(end) = after.find('\'') else {
            break;
        };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out.sort();
    out
}

fn scan_plugin_manifest_level(
    path: &Path,
    source: &str,
    skip_names: &BTreeSet<&str>,
    prefix: &str,
    depth: usize,
) -> io::Result<Vec<Value>> {
    let mut manifests = Vec::new();
    if !path.is_dir() {
        return Ok(manifests);
    }

    let mut children = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.file_name());

    for child in children {
        let child_path = child.path();
        let child_name = child.file_name().to_string_lossy().to_string();
        if depth == 0 && skip_names.contains(child_name.as_str()) {
            continue;
        }

        let yaml_path = child_path.join("plugin.yaml");
        let yml_path = child_path.join("plugin.yml");
        let manifest_file = if yaml_path.exists() {
            Some(yaml_path)
        } else if yml_path.exists() {
            Some(yml_path)
        } else {
            None
        };

        if let Some(manifest_file) = manifest_file {
            if let Some(manifest) =
                parse_plugin_manifest(&manifest_file, &child_path, source, prefix)?
            {
                manifests.push(manifest);
            }
            continue;
        }

        if depth >= 1 {
            continue;
        }
        let sub_prefix = if prefix.is_empty() {
            child_name
        } else {
            format!("{prefix}/{child_name}")
        };
        manifests.extend(scan_plugin_manifest_level(
            &child_path,
            source,
            skip_names,
            &sub_prefix,
            depth + 1,
        )?);
    }

    Ok(manifests)
}

fn parse_plugin_manifest(
    manifest_file: &Path,
    plugin_dir: &Path,
    source: &str,
    prefix: &str,
) -> io::Result<Option<Value>> {
    let text = fs::read_to_string(manifest_file)?;
    let Ok(data) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return Ok(None);
    };
    if !data.is_mapping() {
        return Ok(None);
    }
    let name = yaml_str(&data, "name")
        .map(str::to_string)
        .unwrap_or_else(|| {
            plugin_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
    let key = if prefix.is_empty() {
        name.clone()
    } else {
        format!(
            "{}/{}",
            prefix,
            plugin_dir.file_name().unwrap_or_default().to_string_lossy()
        )
    };

    let mut kind = yaml_str(&data, "kind")
        .unwrap_or("standalone")
        .trim()
        .to_ascii_lowercase();
    if !PLUGIN_VALID_KINDS.contains(&kind.as_str()) {
        kind = "standalone".to_string();
    }

    if kind == "standalone" && !yaml_has_key(&data, "kind") {
        let init_file = plugin_dir.join("__init__.py");
        if let Ok(source_text) = fs::read_to_string(init_file) {
            let source_text = truncate_chars(&source_text, 8192);
            if source_text.contains("register_memory_provider")
                || source_text.contains("MemoryProvider")
            {
                kind = "exclusive".to_string();
            } else if source_text.contains("register_provider")
                && source_text.contains("ProviderProfile")
            {
                kind = "model-provider".to_string();
            }
        }
    }

    Ok(Some(json!({
        "name": name,
        "version": yaml_string_field(&data, "version"),
        "description": yaml_string_field(&data, "description"),
        "author": yaml_string_field(&data, "author"),
        "requires_env": yaml_json_field(&data, "requires_env"),
        "provides_tools": yaml_json_field(&data, "provides_tools"),
        "provides_hooks": yaml_json_field(&data, "provides_hooks"),
        "source": source,
        "path": plugin_dir.to_string_lossy(),
        "kind": kind,
        "key": key,
    })))
}

fn yaml_str<'a>(data: &'a serde_yaml::Value, key: &str) -> Option<&'a str> {
    data.as_mapping()?
        .get(serde_yaml::Value::String(key.to_string()))?
        .as_str()
}

fn yaml_has_key(data: &serde_yaml::Value, key: &str) -> bool {
    data.as_mapping()
        .is_some_and(|map| map.contains_key(serde_yaml::Value::String(key.to_string())))
}

fn yaml_string_field(data: &serde_yaml::Value, key: &str) -> String {
    yaml_str(data, key).unwrap_or("").to_string()
}

fn yaml_json_field(data: &serde_yaml::Value, key: &str) -> Value {
    data.as_mapping()
        .and_then(|map| map.get(serde_yaml::Value::String(key.to_string())))
        .and_then(|value| serde_json::to_value(value).ok())
        .unwrap_or_else(|| json!([]))
}

pub fn dashboard_loopback_hosts() -> &'static [&'static str] {
    &["127.0.0.1", "::1", "localhost", "testclient"]
}

fn details_completion_item(value: &str, meta: &str) -> Value {
    json!({"text": value, "display": value, "meta": meta})
}

fn details_root_completion_item(value: &str, meta: &str, needs_leading_space: bool) -> Value {
    let text = if needs_leading_space {
        format!(" {value}")
    } else {
        value.to_string()
    };
    json!({"text": text, "display": text, "meta": meta})
}

fn url_query_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn split_once_byte(bytes: &[u8], needle: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == needle)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

fn parse_ascii_usize(bytes: &[u8]) -> Option<usize> {
    if !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
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

const TUI_GATEWAY_LONG_HANDLERS: &[&str] = &[
    "browser.manage",
    "cli.exec",
    "session.branch",
    "session.compress",
    "session.resume",
    "shell.exec",
    "skills.manage",
    "slash.exec",
];

const TUI_GATEWAY_METHODS: &[&str] = &[
    "agents.list",
    "approval.respond",
    "browser.manage",
    "clarify.respond",
    "cli.exec",
    "clipboard.paste",
    "command.dispatch",
    "command.resolve",
    "commands.catalog",
    "complete.path",
    "complete.slash",
    "config.get",
    "config.set",
    "config.show",
    "cron.manage",
    "delegation.pause",
    "delegation.status",
    "image.attach",
    "input.detect_drop",
    "insights.get",
    "model.disconnect",
    "model.options",
    "model.save_key",
    "paste.collapse",
    "plugins.list",
    "process.stop",
    "prompt.background",
    "prompt.submit",
    "reload.env",
    "reload.mcp",
    "rollback.diff",
    "rollback.list",
    "rollback.restore",
    "secret.respond",
    "session.branch",
    "session.close",
    "session.compress",
    "session.create",
    "session.delete",
    "session.history",
    "session.interrupt",
    "session.list",
    "session.most_recent",
    "session.resume",
    "session.save",
    "session.status",
    "session.steer",
    "session.title",
    "session.undo",
    "session.usage",
    "setup.status",
    "shell.exec",
    "skills.manage",
    "skills.reload",
    "slash.exec",
    "spawn_tree.list",
    "spawn_tree.load",
    "spawn_tree.save",
    "subagent.interrupt",
    "sudo.respond",
    "terminal.resize",
    "tools.configure",
    "tools.list",
    "tools.show",
    "toolsets.list",
    "voice.record",
    "voice.toggle",
    "voice.tts",
];

const DETAIL_SECTION_NAMES: &[&str] = &["thinking", "tools", "subagents", "activity"];
const DETAIL_MODES: &[&str] = &["hidden", "collapsed", "expanded"];

const PLUGIN_VALID_KINDS: &[&str] = &[
    "backend",
    "exclusive",
    "model-provider",
    "platform",
    "standalone",
];

const PLUGIN_VALID_HOOKS: &[&str] = &[
    "on_session_end",
    "on_session_finalize",
    "on_session_reset",
    "on_session_start",
    "post_api_request",
    "post_approval_response",
    "post_llm_call",
    "post_tool_call",
    "pre_api_request",
    "pre_approval_request",
    "pre_gateway_dispatch",
    "pre_llm_call",
    "pre_tool_call",
    "subagent_stop",
    "transform_llm_output",
    "transform_terminal_output",
    "transform_tool_result",
];

const DEFAULT_VOICE_PT_KEY: &str = "c-b";
const VOICE_RESERVED_CTRL_CHARS: &[&str] = &["c", "d", "l"];

pub fn voice_record_key_from_config(cfg: &Value) -> Value {
    cfg.get("voice")
        .and_then(Value::as_object)
        .and_then(|voice| voice.get("record_key"))
        .cloned()
        .unwrap_or(Value::Null)
}

pub fn normalize_voice_record_key_for_prompt_toolkit(raw: &Value) -> String {
    let Some(raw) = raw.as_str() else {
        return DEFAULT_VOICE_PT_KEY.to_string();
    };
    let lowered = raw.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return DEFAULT_VOICE_PT_KEY.to_string();
    }

    let parts = lowered
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 2 {
        return DEFAULT_VOICE_PT_KEY.to_string();
    }

    let modifier = parts[0];
    let key = parts[1];
    if matches!(modifier, "super" | "win" | "windows") {
        return DEFAULT_VOICE_PT_KEY.to_string();
    }

    let normalized_mod = match modifier {
        "ctrl" | "control" => "c-",
        "alt" | "option" | "opt" => "a-",
        _ => return DEFAULT_VOICE_PT_KEY.to_string(),
    };

    if key.chars().count() == 1 {
        if normalized_mod == "c-" && VOICE_RESERVED_CTRL_CHARS.contains(&key) {
            return DEFAULT_VOICE_PT_KEY.to_string();
        }
        return format!("{normalized_mod}{key}");
    }

    let named = match key {
        "space" | "spc" => "space",
        "enter" | "return" | "ret" => "enter",
        "tab" => "tab",
        "escape" | "esc" => "escape",
        "backspace" | "bs" => "backspace",
        "delete" | "del" => "delete",
        _ => return DEFAULT_VOICE_PT_KEY.to_string(),
    };
    format!("{normalized_mod}{named}")
}

pub fn format_voice_record_key_for_status(raw: &Value) -> String {
    let normalized = normalize_voice_record_key_for_prompt_toolkit(raw);
    let (prefix, key) = if let Some(key) = normalized.strip_prefix("c-") {
        ("Ctrl+", key)
    } else if let Some(key) = normalized.strip_prefix("a-") {
        ("Alt+", key)
    } else if let Some((modifier, key)) = normalized.split_once('+') {
        let mut chars = modifier.chars();
        let prefix = match chars.next() {
            Some(first) => format!("{}{}+", first.to_ascii_uppercase(), chars.as_str()),
            None => "+".to_string(),
        };
        return format!("{prefix}{}", title_case_key(key));
    } else {
        return "Ctrl+B".to_string();
    };

    if key.is_empty() {
        return prefix.trim_end_matches('+').to_string();
    }
    format!("{prefix}{}", title_case_key(key))
}

fn title_case_key(key: &str) -> String {
    if key.chars().count() == 1 {
        return key.to_ascii_uppercase();
    }
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

pub fn voice_record_key_config_fixture(cases: &[Value]) -> Value {
    Value::Array(
        cases
            .iter()
            .map(|case| {
                let cfg = &case["config"];
                json!({
                    "config": cfg,
                    "record_key": voice_record_key_from_config(cfg),
                })
            })
            .collect(),
    )
}

pub fn voice_record_key_normalization_fixture(cases: &[Value]) -> Value {
    Value::Array(
        cases
            .iter()
            .map(|case| {
                let raw = &case["raw"];
                json!({
                    "normalized": normalize_voice_record_key_for_prompt_toolkit(raw),
                    "raw": raw,
                    "status": format_voice_record_key_for_status(raw),
                })
            })
            .collect(),
    )
}

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
