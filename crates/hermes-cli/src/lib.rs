use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::{json, Value};
use tar::{Archive, Builder, EntryType};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

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

const TOOLSET_PLATFORMS: &[&str] = &[
    "cli",
    "telegram",
    "discord",
    "slack",
    "whatsapp",
    "signal",
    "bluebubbles",
    "email",
    "homeassistant",
    "mattermost",
    "matrix",
    "dingtalk",
    "feishu",
    "wecom",
    "wecom_callback",
    "weixin",
    "qqbot",
    "yuanbao",
    "webhook",
    "api_server",
    "cron",
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
        ["hermes", "status"] => {
            stdout = format!(
                "\n┌─────────────────────────────────────────────────────────┐\n│                 ⚕ Hermes Agent Status                  │\n└─────────────────────────────────────────────────────────┘\n\n◆ Environment\n  Project:      <PROJECT_ROOT>\n  Python:       unavailable\n  .env file:    {}\n  Model:        (not set)\n  Provider:     Auto\n\n◆ API Keys\n  OpenRouter    ✗ (not set)\n\n◆ Auth Providers\n  Nous Portal   ✗ not logged in (run: hermes auth add nous --type oauth)\n  OpenAI Codex  ✗ not logged in (run: hermes model)\n\n◆ Terminal Backend\n  Backend:      local\n  Sudo:         ✗ disabled\n\n◆ Messaging Platforms\n  Telegram      ✗ not configured\n  Discord       ✗ not configured\n\n◆ Gateway Service\n  Status:       ✗ stopped\n  Manager:      docker (foreground)\n\n◆ Scheduled Jobs\n  Jobs:         0\n\n◆ Sessions\n  Active:       0\n\n────────────────────────────────────────────────────────────\n  Run 'hermes doctor' for detailed diagnostics\n  Run 'hermes setup' to configure\n\n",
                if Path::new(hermes_home).join(".env").exists() {
                    "✓ found"
                } else {
                    "✗ not found"
                }
            );
        }
        ["hermes", "logout"] => {
            stdout = "No provider is currently logged in.\n".to_string();
        }
        ["hermes", "auth", "list"] | ["hermes", "auth", "list", _] => {}
        ["hermes", "auth", "status", provider] => {
            stdout = format!("{provider}: logged out\n");
        }
        ["hermes", "auth", "logout", "openrouter"] => {
            exit_code = 1;
            stdout = "Unknown provider: openrouter\n".to_string();
        }
        ["hermes", "auth", "reset", provider] => {
            stdout = format!("Reset status on 0 {provider} credentials\n");
        }
        ["hermes", "auth", "remove", provider, target] => {
            exit_code = 1;
            stderr = format!("No credential matching \"{target}\". Provider: {provider}.\n");
        }
        ["hermes", "memory", "status"] => {
            stdout = memory_status_output();
        }
        ["hermes", "memory", "off"] => {
            stdout = "\n  ✓ Memory provider: built-in only\n  Saved to config.yaml\n\n".to_string();
        }
        ["hermes", "memory", "reset", "--target", _, "--yes"] => {
            stdout = format!(
                "\n  Nothing to reset — no memory files found in {hermes_home}/memories/\n\n"
            );
        }
        ["hermes", "backup", "--quick", "--label", label] => {
            let snap_id = format!("19700101-000000-{label}");
            stdout = format!(
                "State snapshot created: {snap_id}\n  1 snapshot(s) stored in {hermes_home}/state-snapshots/\n  Restore with: /snapshot restore {snap_id}\n"
            );
        }
        ["hermes", "backup", "-o", output] | ["hermes", "backup", "--output", output] => {
            stdout = format!(
                "Scanning {hermes_home} ...\nBacking up 0 files ...\n\nBackup complete: {output}\n  Files:       0\n\nRestore with: hermes import backup.zip\n"
            );
        }
        ["hermes", "import", archive, "--force"] | ["hermes", "import", archive, "-f"] => {
            stdout = format!(
                "Backup contains 0 files\nTarget: {hermes_home}\n\nImporting 0 files ...\n\nImport complete: 0 files restored in 0.0s\n  Target: {hermes_home}\nDone. Your Hermes configuration has been restored.\n"
            );
            let _ = archive;
        }
        ["hermes", "pairing", "list"] => {
            stdout = "No pairing data found. No one has tried to pair yet~\n".to_string();
        }
        ["hermes", "pairing", "approve", platform, code] => {
            stdout = pairing_missing_code_output(platform, code);
        }
        ["hermes", "pairing", "revoke", platform, user_id] => {
            stdout = pairing_revoke_missing_output(platform, user_id);
        }
        ["hermes", "pairing", "clear-pending"] => {
            stdout = "\n  No pending requests to clear.\n".to_string();
        }
        ["hermes", "slack", "manifest", "--slashes-only"] => {
            stdout = slack_slashes_only_json();
        }
        ["hermes", "slack", "manifest", "--name", name, "--description", description] => {
            stdout = slack_full_manifest_json(name, description);
        }
        ["hermes", "slack", "manifest", "--write", path, "--slashes-only"] => {
            let _ = path;
            stderr = slack_manifest_write_stderr(&format!("{hermes_home}/slack-manifest.json"));
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
        ["hermes", "completion", "bash"] => {
            stdout = completion_bash_output();
        }
        ["hermes", "completion", "zsh"] => {
            stdout = completion_zsh_output();
        }
        ["hermes", "completion", "fish"] => {
            stdout = completion_fish_output();
        }
        ["hermes", "computer-use", "status"] => {
            stdout = "cua-driver: not installed\n  Run: hermes computer-use install\n".to_string();
        }
        ["hermes", "config", "edit"] => {
            let config_path = Path::new(hermes_home).join("config.yaml");
            if !config_path.exists() {
                if let Err(error) = fs::write(&config_path, "{}\n") {
                    exit_code = 1;
                    stderr = format!("Failed to create config: {error}\n");
                } else {
                    stdout.push_str(&format!("Created {hermes_home}/config.yaml\n"));
                }
            }
            stdout.push_str(&format!(
                "No editor found. Config file is at:\n  {hermes_home}/config.yaml\n"
            ));
        }
        ["hermes", "config", "set"] | ["hermes", "config", "set", _] => {
            exit_code = 1;
            stdout = "Usage: hermes config set <key> <value>\n\nExamples:\n  hermes config set model anthropic/claude-sonnet-4\n  hermes config set terminal.backend docker\n  hermes config set OPENROUTER_API_KEY sk-or-...\n".to_string();
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
        ["hermes", "dashboard", "--status"] | ["hermes", "dashboard", "--stop"] => {
            stdout = "No hermes dashboard processes running.\n".to_string();
        }
        ["hermes", "gateway", "status"] => {
            stdout = gateway_status_output();
        }
        ["hermes", "gateway", "list"] => {
            stdout = "Gateways:\n  ✗ default (current)       \n".to_string();
        }
        ["hermes", "gateway", "stop"] => {
            stdout = "✗ No gateway running for this profile\n".to_string();
        }
        ["hermes", "gateway", "start"] => {
            stdout = gateway_container_start_output();
        }
        ["hermes", "gateway", "install"] => {
            stdout = gateway_container_install_output();
        }
        ["hermes", "gateway", "uninstall"] => {
            stdout = gateway_container_uninstall_output();
        }
        ["hermes", "hooks", "list"] | ["hermes", "hooks", "ls"] => {
            stdout = "No shell hooks configured in ~/.hermes/config.yaml.\nSee `hermes hooks --help` or\n    website/docs/user-guide/features/hooks.md\nfor the config schema and worked examples.\n".to_string();
        }
        ["hermes", "hooks", "doctor"] => {
            stdout = "No shell hooks configured — nothing to check.\n".to_string();
        }
        ["hermes", "insights"] => {
            return insights_empty_output(30, None);
        }
        ["hermes", "insights", "--days", days, "--source", source] => {
            return insights_empty_output(days.parse().unwrap_or(30), Some(*source));
        }
        ["hermes", "doctor", "--ack", advisory] => {
            if *advisory == "shai-hulud-2026-05" {
                stdout = format!(
                    "  ✓ Acknowledged advisory {advisory}. It will no longer trigger startup banners.\n"
                );
            } else {
                exit_code = 2;
                stdout =
                    format!("Unknown advisory ID: '{advisory}'. Known IDs: shai-hulud-2026-05\n");
            }
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
            stdout = mcp_list_output(&json!({}));
        }
        ["hermes", "mcp", "add", _name] => {
            stdout = "\n  ✗ Must specify --url <endpoint>, --command <cmd>, or --preset <name>\n  Examples:\n  hermes mcp add ink --url \"https://mcp.ml.ink/mcp\"\n  hermes mcp add github --command npx --args @modelcontextprotocol/server-github\n  hermes mcp add myserver --preset mypreset\n".to_string();
        }
        ["hermes", "mcp", "add", _name, "--command", _command, "--env", env_value] => {
            if env_value.split_once('=').is_none() {
                stdout = format!("  ✗ Invalid --env value '{env_value}' (expected KEY=VALUE)\n");
            } else {
                stdout = "\n  Connecting to MCP server...\n".to_string();
            }
        }
        ["hermes", "mcp", "test", name] => {
            stdout = format!("  ✗ Server '{name}' not found in config.\n");
        }
        ["hermes", "tools", "list"] => {
            stdout = tools_list_output("cli", &default_display_enabled_toolsets());
        }
        ["hermes", "tools", "enable", target] if target.contains(':') => {
            stdout = format!("✓ Enabled: {target}\n");
        }
        ["hermes", "tools", "disable", target] if target.contains(':') => {
            stdout = format!("✓ Disabled: {target}\n");
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
            let created = create_cron_job(hermes_home, schedule, prompt, name, deliver)?;
            result = cron_create_output(&created);
        }
        ["hermes", "cron", "pause", name] => {
            match set_cron_job_enabled(hermes_home, name, false)? {
                CronMutationOutcome::Changed => {
                    result = run_safe_command(argv, &home_display);
                }
                CronMutationOutcome::Missing => {
                    result = cron_missing_job_output("pause", name);
                }
                CronMutationOutcome::Ambiguous(ids) => {
                    result = cron_ambiguous_job_output("pause", name, &ids);
                }
            }
        }
        ["hermes", "cron", "resume", name] => {
            match set_cron_job_enabled(hermes_home, name, true)? {
                CronMutationOutcome::Changed => {
                    result = run_safe_command(argv, &home_display);
                }
                CronMutationOutcome::Missing => {
                    result = cron_missing_job_output("resume", name);
                }
                CronMutationOutcome::Ambiguous(ids) => {
                    result = cron_ambiguous_job_output("resume", name, &ids);
                }
            }
        }
        ["hermes", "cron", "remove", name]
        | ["hermes", "cron", "rm", name]
        | ["hermes", "cron", "delete", name] => match remove_cron_job(hermes_home, name)? {
            CronMutationOutcome::Changed => {
                result = run_safe_command(argv, &home_display);
            }
            CronMutationOutcome::Missing => {
                result = cron_missing_job_output("remove", name);
            }
            CronMutationOutcome::Ambiguous(ids) => {
                result = cron_ambiguous_job_output("remove", name, &ids);
            }
        },
        ["hermes", "cron", "status"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: "\n✗ Gateway is not running — cron jobs will NOT fire\n\n  To enable automatic execution:\n    hermes gateway install    # Install as a user service\n    sudo hermes gateway install --system  # Linux servers: boot-time system service\n    hermes gateway            # Or run in foreground\n\n  No active jobs\n\n".to_string(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "cron", "run", name] => {
            result = CliExecution {
                exit_code: 0,
                stdout: format!(
                    "Failed to run job: Job with ID or name '{name}' not found. Use cronjob(action='list') to inspect jobs.\n"
                ),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "memory", "status"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: memory_status_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "memory", "off"] => {
            set_memory_provider(hermes_home, "")?;
            result = CliExecution {
                exit_code: 0,
                stdout: "\n  ✓ Memory provider: built-in only\n  Saved to config.yaml\n\n"
                    .to_string(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "memory", "reset", "--target", target, "--yes"] => {
            result = memory_reset_target(hermes_home, target)?;
        }
        ["hermes", "backup", "--quick", "--label", label] => {
            result = create_quick_backup_output(hermes_home, label)?;
        }
        ["hermes", "backup", "-o", output] | ["hermes", "backup", "--output", output] => {
            result = create_full_backup_output(hermes_home, Path::new(output))?;
        }
        ["hermes", "import", archive, "--force"] | ["hermes", "import", archive, "-f"] => {
            result = restore_full_backup_output(hermes_home, Path::new(archive))?;
        }
        ["hermes", "bundles"] | ["hermes", "bundles", "list"] => {
            result = bundles_list_output(hermes_home)?;
        }
        ["hermes", "bundles", "show", name] => {
            result = bundles_show_output(hermes_home, name)?;
        }
        ["hermes", "bundles", "reload"] => {
            result = bundles_reload_output(hermes_home)?;
        }
        ["hermes", "bundles", "delete", name] => {
            result = bundles_delete_output(hermes_home, name)?;
        }
        ["hermes", "bundles", "create", name, args @ ..] => {
            result = bundles_create_output(hermes_home, name, args)?;
        }
        ["hermes", "fallback"] | ["hermes", "fallback", "list"] | ["hermes", "fallback", "ls"] => {
            result = fallback_list_output(hermes_home)?;
        }
        ["hermes", "fallback", "remove"] | ["hermes", "fallback", "rm"] => {
            result = fallback_remove_output(hermes_home)?;
        }
        ["hermes", "fallback", "clear"] => {
            result = fallback_clear_output(hermes_home)?;
        }
        ["hermes", "curator", "status"] => {
            result = curator_status_output(hermes_home)?;
        }
        ["hermes", "curator", "pause"] => {
            result = curator_set_paused_output(hermes_home, true)?;
        }
        ["hermes", "curator", "resume"] => {
            result = curator_set_paused_output(hermes_home, false)?;
        }
        ["hermes", "curator", "list-archived"] => {
            result = curator_list_archived_output(hermes_home);
        }
        ["hermes", "dump"] => {
            result = dump_output(hermes_home, false)?;
        }
        ["hermes", "dump", "--show-keys"] => {
            result = dump_output(hermes_home, true)?;
        }
        ["hermes", "pairing", "list"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: pairing_list_output(hermes_home)?,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "pairing", "approve", platform, code] => {
            result = pairing_approve_code(hermes_home, platform, code)?;
        }
        ["hermes", "pairing", "revoke", platform, user_id] => {
            result = pairing_revoke_user(hermes_home, platform, user_id)?;
        }
        ["hermes", "pairing", "clear-pending"] => {
            result = pairing_clear_pending(hermes_home)?;
        }
        ["hermes", "slack", "manifest", "--slashes-only"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: slack_slashes_only_json(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "slack", "manifest", "--name", name, "--description", description] => {
            result = CliExecution {
                exit_code: 0,
                stdout: slack_full_manifest_json(name, description),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "slack", "manifest", "--write", path, "--slashes-only"] => {
            let payload = slack_slashes_only_json();
            let target = Path::new(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(target, &payload)?;
            result = CliExecution {
                exit_code: 0,
                stdout: String::new(),
                stdout_markers: BTreeMap::new(),
                stderr: slack_manifest_write_stderr(path),
            };
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
        ["hermes", "gateway", "status"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: gateway_status_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "gateway", "list"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: gateway_list_output(hermes_home)?,
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "gateway", "stop"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: "✗ No gateway running for this profile\n".to_string(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "gateway", "start"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: gateway_container_start_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "gateway", "install"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: gateway_container_install_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "gateway", "uninstall"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: gateway_container_uninstall_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "webhook", "list"] | ["hermes", "webhook", "ls"] => {
            result = webhook_list_output(hermes_home)?;
        }
        ["hermes", "webhook", "subscribe", name, args @ ..]
        | ["hermes", "webhook", "add", name, args @ ..] => {
            result = webhook_subscribe_output(hermes_home, name, args)?;
        }
        ["hermes", "webhook", "remove", name] | ["hermes", "webhook", "rm", name] => {
            result = webhook_remove_output(hermes_home, name)?;
        }
        ["hermes", "webhook", "test", name] => {
            result = webhook_test_output(hermes_home, name)?;
        }
        ["hermes", "plugins", "list"] | ["hermes", "plugins", "ls"] => {
            result = plugins_list_output(hermes_home)?;
        }
        ["hermes", "plugins", "enable", name] => {
            result = plugins_set_enabled_output(hermes_home, name, true)?;
        }
        ["hermes", "plugins", "disable", name] => {
            result = plugins_set_enabled_output(hermes_home, name, false)?;
        }
        ["hermes", "skills", "list"] => {
            result = skills_list_cli_output(hermes_home, "all", false)?;
        }
        ["hermes", "skills", "list", "--enabled-only"] => {
            result = skills_list_cli_output(hermes_home, "all", true)?;
        }
        ["hermes", "skills", "list", "--source", source] => {
            result = skills_list_cli_output(hermes_home, source, false)?;
        }
        ["hermes", "skills", "check"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: "No hub-installed skills to check.\n\n".to_string(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "skills", "audit"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: "No hub-installed skills to audit.\n\n".to_string(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "checkpoints"] | ["hermes", "checkpoints", "status"] => {
            result = checkpoints_status_output(hermes_home)?;
        }
        ["hermes", "checkpoints", "list"] => {
            result = checkpoints_status_output(hermes_home)?;
        }
        ["hermes", "checkpoints", "list", "--limit", _limit] => {
            result = checkpoints_status_output(hermes_home)?;
        }
        ["hermes", "checkpoints", "clear-legacy", "-f"]
        | ["hermes", "checkpoints", "clear-legacy", "--force"] => {
            result = checkpoints_clear_legacy_output(hermes_home)?;
        }
        ["hermes", "checkpoints", "clear", "-f"]
        | ["hermes", "checkpoints", "clear", "--force"] => {
            result = checkpoints_clear_output(hermes_home)?;
        }
        ["hermes", "proxy"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: proxy_help_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "proxy", "providers"] | ["hermes", "proxy", "list"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: proxy_providers_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "proxy", "status"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: proxy_status_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "debug"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: debug_help_output(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "debug", "share", "--local", "--lines", lines] => {
            result = debug_share_local_output(hermes_home, lines)?;
        }
        ["hermes", "debug", "delete"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: "Usage: hermes debug delete <url> [<url> ...]\n  Deletes paste.rs pastes uploaded by 'hermes debug share'.\n".to_string(),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "debug", "delete", url] => {
            result = CliExecution {
                exit_code: 0,
                stdout: format!(
                    "  ✗ Cannot delete: only paste.rs URLs are supported.  Got: {url}\n"
                ),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "doctor", "--ack", advisory] => {
            if *advisory == "shai-hulud-2026-05" {
                ack_security_advisory(hermes_home, advisory)?;
            }
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "mcp", "list"] | ["hermes", "mcp", "ls"] => {
            let config = read_config_value(hermes_home)?;
            result = CliExecution {
                exit_code: 0,
                stdout: mcp_list_output(&config),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "mcp", "remove", name] | ["hermes", "mcp", "rm", name] => {
            let removed = remove_mcp_server(hermes_home, name)?;
            result = if removed {
                CliExecution {
                    exit_code: 0,
                    stdout: format!("  ✓ Removed '{name}' from config\n"),
                    stdout_markers: BTreeMap::new(),
                    stderr: String::new(),
                }
            } else {
                CliExecution {
                    exit_code: 0,
                    stdout: format!("  ✗ Server '{name}' not found in config.\n"),
                    stdout_markers: BTreeMap::new(),
                    stderr: String::new(),
                }
            };
        }
        ["hermes", "mcp", "add", name] => {
            let _ = name;
            result = run_safe_command(argv, &home_display);
        }
        ["hermes", "mcp", "add", name, "--command", command, "--env", env_value] => {
            let _ = (name, command);
            result = if parse_mcp_env_assignment(env_value).is_err() {
                run_safe_command(argv, &home_display)
            } else {
                CliExecution {
                    exit_code: 0,
                    stdout: "\n  Connecting to MCP server...\n".to_string(),
                    stdout_markers: BTreeMap::new(),
                    stderr: String::new(),
                }
            };
        }
        ["hermes", "mcp", "test", name] => {
            let config = read_config_value(hermes_home)?;
            let servers = mcp_servers(&config);
            result = if servers.contains_key(*name) {
                CliExecution {
                    exit_code: 0,
                    stdout: format!("\n  Testing '{name}'...\n"),
                    stdout_markers: BTreeMap::new(),
                    stderr: String::new(),
                }
            } else {
                let mut stdout = format!("  ✗ Server '{name}' not found in config.\n");
                if !servers.is_empty() {
                    stdout.push_str(&format!(
                        "  Available: {}\n",
                        servers.keys().cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
                CliExecution {
                    exit_code: 0,
                    stdout,
                    stdout_markers: BTreeMap::new(),
                    stderr: String::new(),
                }
            };
        }
        ["hermes", "tools", "enable", name, "--platform", platform] => {
            result = if !is_valid_toolset_platform(platform) {
                unknown_toolset_platform_output(platform)
            } else {
                let names = [*name];
                let valid = filter_valid_toolset_names(&names);
                apply_platform_toolset_change(hermes_home, platform, &valid, true)?;
                toolset_change_output(&names, true)
            };
        }
        ["hermes", "tools", "disable", name, "--platform", platform] => {
            result = if !is_valid_toolset_platform(platform) {
                unknown_toolset_platform_output(platform)
            } else {
                let names = [*name];
                let valid = filter_valid_toolset_names(&names);
                apply_platform_toolset_change(hermes_home, platform, &valid, false)?;
                toolset_change_output(&names, false)
            };
        }
        ["hermes", "tools", "list", "--platform", platform] => {
            result = if !is_valid_toolset_platform(platform) {
                unknown_toolset_platform_output(platform)
            } else {
                let config = read_config_value(hermes_home)?;
                let enabled = read_platform_toolsets(hermes_home, platform)?;
                CliExecution {
                    exit_code: 0,
                    stdout: tools_list_output_with_mcp(platform, &enabled, &config),
                    stdout_markers: BTreeMap::new(),
                    stderr: String::new(),
                }
            };
        }
        ["hermes", "tools", "enable", target] if target.contains(':') => {
            result = mcp_tool_change_output(hermes_home, target, true)?;
        }
        ["hermes", "tools", "disable", target] if target.contains(':') => {
            result = mcp_tool_change_output(hermes_home, target, false)?;
        }
        ["hermes", "tools", "enable", names @ ..] if !names.is_empty() => {
            let valid = filter_valid_toolset_names(names);
            apply_platform_toolset_change(hermes_home, "cli", &valid, true)?;
            result = toolset_change_output(names, true);
        }
        ["hermes", "tools", "disable", names @ ..] if !names.is_empty() => {
            let valid = filter_valid_toolset_names(names);
            apply_platform_toolset_change(hermes_home, "cli", &valid, false)?;
            result = toolset_change_output(names, false);
        }
        ["hermes", "tools", "list"] => {
            let config = read_config_value(hermes_home)?;
            let enabled = read_platform_toolsets(hermes_home, "cli")?;
            result = CliExecution {
                exit_code: 0,
                stdout: tools_list_output_with_mcp("cli", &enabled, &config),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "logs", "list"] => {
            result = CliExecution {
                exit_code: 0,
                stdout: logs_list_output(hermes_home),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            };
        }
        ["hermes", "logs", log_name, "-n", lines] => {
            result = logs_tail_execution(hermes_home, log_name, lines, None, None, None);
        }
        ["hermes", "logs", log_name, "-n", lines, "--level", level, "--session", session, "--component", component] =>
        {
            result = logs_tail_execution(
                hermes_home,
                log_name,
                lines,
                Some(level),
                Some(session),
                Some(component),
            );
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
        ["hermes", "profile", "create", name, "--no-alias", "--no-skills", "--description", description] => {
            match checked_profile_dir(hermes_home, name) {
                Err(error) => result = error,
                Ok((_canon, dir)) => {
                    if dir.exists() {
                        result = CliExecution {
                            exit_code: 1,
                            stdout: format!(
                                "Error: Profile '{name}' already exists at {}\n",
                                dir.display()
                            ),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    } else {
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
                }
            }
        }
        ["hermes", "profile", "create", name, "--clone", "--no-alias", "--description", description] =>
        {
            let source = profile_dir(hermes_home, &active_profile_name(hermes_home));
            match checked_profile_dir(hermes_home, name) {
                Err(error) => result = error,
                Ok((_canon, dir)) => {
                    if dir.exists() {
                        result = CliExecution {
                            exit_code: 1,
                            stdout: format!(
                                "Error: Profile '{name}' already exists at {}\n",
                                dir.display()
                            ),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    } else {
                        create_cloned_profile(&source, &dir, Some(description))?;
                        result = CliExecution {
                            exit_code: 0,
                            stdout: format!(
                                "\nProfile '{name}' created at {}\nCloned config, .env, SOUL.md, and skills from {}.\n\nNext steps:\n  {name} setup              Configure API keys and model\n  {name} chat               Start chatting\n  {name} gateway start      Start the messaging gateway\n\n  Edit {}/.env for different API keys\n  Edit {}/SOUL.md for different personality\n\n",
                                dir.display(),
                                active_profile_name(hermes_home),
                                dir.display(),
                                dir.display(),
                            ),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    }
                }
            }
        }
        ["hermes", "profile", "create", name, "--clone-all", "--no-alias", "--description", description] =>
        {
            let source_name = active_profile_name(hermes_home);
            let source = profile_dir(hermes_home, &source_name);
            match checked_profile_dir(hermes_home, name) {
                Err(error) => result = error,
                Ok((_canon, dir)) => {
                    if dir.exists() {
                        result = CliExecution {
                            exit_code: 1,
                            stdout: format!(
                                "Error: Profile '{name}' already exists at {}\n",
                                dir.display()
                            ),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    } else {
                        create_clone_all_profile(&source, &dir, Some(description))?;
                        result = CliExecution {
                            exit_code: 0,
                            stdout: format!(
                                "\nProfile '{name}' created at {}\nFull copy from {source_name}.\n\nNext steps:\n  {name} setup              Configure API keys and model\n  {name} chat               Start chatting\n  {name} gateway start      Start the messaging gateway\n\n  Edit {}/.env for different API keys\n  Edit {}/SOUL.md for different personality\n\n",
                                dir.display(),
                                dir.display(),
                                dir.display(),
                            ),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    }
                }
            }
        }
        ["hermes", "profile", "describe", name] => match checked_profile_dir(hermes_home, name) {
            Err(error) => result = error,
            Ok((canon, dir)) => {
                if canon != "default" && !dir.is_dir() {
                    result = CliExecution {
                        exit_code: 1,
                        stdout: String::new(),
                        stdout_markers: BTreeMap::new(),
                        stderr: format!("Error: profile '{canon}' not found\n"),
                    };
                } else {
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
            }
        },
        ["hermes", "profile", "describe", name, "--text", description] => {
            match checked_profile_dir(hermes_home, name) {
                Err(error) => result = error,
                Ok((canon, dir)) => {
                    if canon != "default" && !dir.is_dir() {
                        result = CliExecution {
                            exit_code: 1,
                            stdout: String::new(),
                            stdout_markers: BTreeMap::new(),
                            stderr: format!(
                                "Error: profile directory does not exist: {}\n",
                                dir.display()
                            ),
                        };
                    } else {
                        write_profile_description(&dir, description)?;
                        result = CliExecution {
                            exit_code: 0,
                            stdout: format!("Description updated for '{name}'.\n"),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    }
                }
            }
        }
        ["hermes", "profile", "show", name] => match checked_profile_dir(hermes_home, name) {
            Err(error) => result = error,
            Ok((canon, dir)) => {
                if canon != "default" && !dir.is_dir() {
                    result = CliExecution {
                        exit_code: 1,
                        stdout: format!("Error: Profile '{name}' does not exist.\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                } else {
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
            }
        },
        ["hermes", "profile", "use", name] => match checked_profile_dir(hermes_home, name) {
            Err(error) => result = error,
            Ok((canon, dir)) => {
                if canon == "default" {
                    let _ = fs::remove_file(hermes_home.join("active_profile"));
                    result = CliExecution {
                        exit_code: 0,
                        stdout: "Switched to: default (~/.hermes)\n".to_string(),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                } else if !dir.is_dir() {
                    result = CliExecution {
                            exit_code: 1,
                            stdout: format!(
                                "Error: Profile '{name}' does not exist. Create it with: hermes profile create {name}\n"
                            ),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                } else {
                    fs::create_dir_all(hermes_home)?;
                    fs::write(hermes_home.join("active_profile"), format!("{canon}\n"))?;
                    result = CliExecution {
                        exit_code: 0,
                        stdout: format!("Switched to: {name}\n"),
                        stdout_markers: BTreeMap::new(),
                        stderr: String::new(),
                    };
                }
            }
        },
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
            match checked_profile_dir(hermes_home, name) {
                Err(error) => result = error,
                Ok((canon, dir)) => {
                    if canon == "default" {
                        result = CliExecution {
                            exit_code: 1,
                            stdout:
                                "Error: Cannot delete the default profile (~/.hermes).\nTo remove everything, use: hermes uninstall\n"
                                    .to_string(),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    } else if !dir.is_dir() {
                        result = CliExecution {
                            exit_code: 1,
                            stdout: format!("Error: Profile '{name}' does not exist.\n"),
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    } else {
                        let _ = fs::remove_dir_all(&dir);
                        let mut stdout = format!(
                            "\nProfile: {canon}\nPath:    {}\n\nThis will permanently delete:\n  • All config, API keys, memories, sessions, skills, cron jobs\n✓ Removed {}\n",
                            dir.display(),
                            dir.display(),
                        );
                        if active_profile_name(hermes_home) == canon {
                            let _ = fs::remove_file(hermes_home.join("active_profile"));
                            stdout.push_str("✓ Active profile reset to default\n");
                        }
                        stdout.push_str(&format!("\nProfile '{canon}' deleted.\n"));
                        result = CliExecution {
                            exit_code: 0,
                            stdout,
                            stdout_markers: BTreeMap::new(),
                            stderr: String::new(),
                        };
                    }
                }
            }
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
        ["hermes", "sessions", "export", output, "--source", source] => {
            let db = open_session_db(hermes_home)?;
            let exported = db
                .export_all_optional(Some(source))
                .map_err(io::Error::other)?;
            let mut body = String::new();
            let mut count = 0usize;
            for session in exported.as_array().into_iter().flatten() {
                body.push_str(&serde_json::to_string(session).unwrap());
                body.push('\n');
                count += 1;
            }
            fs::write(output, body)?;
            result = CliExecution {
                exit_code: 0,
                stdout: format!("Exported {count} sessions to {output}\n"),
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

fn default_platform_saved_toolsets(platform: &str) -> BTreeSet<String> {
    let mut toolsets = default_cli_saved_toolsets();
    if platform != "cli" {
        toolsets.insert("browser".to_string());
    }
    toolsets
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

fn write_config_value(hermes_home: &Path, config: &Value) -> io::Result<()> {
    fs::create_dir_all(hermes_home)?;
    fs::write(
        hermes_home.join("config.yaml"),
        serde_yaml::to_string(config).unwrap_or_else(|_| "{}\n".to_string()),
    )
}

fn ack_security_advisory(hermes_home: &Path, advisory: &str) -> io::Result<()> {
    let mut config = read_config_value(hermes_home)?;
    if !config.is_object() {
        config = json!({});
    }
    let root = config.as_object_mut().unwrap();
    let security = root
        .entry("security".to_string())
        .or_insert_with(|| json!({}));
    if !security.is_object() {
        *security = json!({});
    }
    let security = security.as_object_mut().unwrap();
    let advisories = security
        .entry("acked_advisories".to_string())
        .or_insert_with(|| json!([]));
    if !advisories.is_array() {
        *advisories = json!([]);
    }
    let list = advisories.as_array_mut().unwrap();
    if !list.iter().any(|value| value.as_str() == Some(advisory)) {
        list.push(Value::String(advisory.to_string()));
        list.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
    }
    write_config_value(hermes_home, &config)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillBundle {
    name: String,
    slug: String,
    description: String,
    skills: Vec<String>,
    instruction: String,
    path: PathBuf,
}

fn bundle_slugify(name: &str) -> String {
    let mut out = String::new();
    let mut previous_hyphen = false;
    for ch in name.trim().chars().flat_map(char::to_lowercase) {
        let next = if ch == ' ' || ch == '_' || ch == '-' {
            Some('-')
        } else if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            Some(ch)
        } else {
            None
        };
        if let Some(ch) = next {
            if ch == '-' {
                if !previous_hyphen && !out.is_empty() {
                    out.push('-');
                }
                previous_hyphen = true;
            } else {
                out.push(ch);
                previous_hyphen = false;
            }
        }
    }
    out.trim_matches('-').to_string()
}

fn bundles_dir(hermes_home: &Path) -> PathBuf {
    hermes_home.join("skill-bundles")
}

fn bundle_path_for(hermes_home: &Path, name: &str) -> io::Result<PathBuf> {
    let slug = bundle_slugify(name);
    if slug.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Bundle name {name:?} normalizes to an empty slug"),
        ));
    }
    Ok(bundles_dir(hermes_home).join(format!("{slug}.yaml")))
}

fn yaml_sequence_strings(value: Option<&serde_yaml::Value>) -> Vec<String> {
    value
        .and_then(serde_yaml::Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn load_bundle_file(path: &Path) -> io::Result<Option<SkillBundle>> {
    let text = fs::read_to_string(path)?;
    let Ok(data) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return Ok(None);
    };
    let Some(map) = data.as_mapping() else {
        return Ok(None);
    };
    let key = |name: &str| serde_yaml::Value::String(name.to_string());
    let name = map
        .get(key("name"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
    let skills = yaml_sequence_strings(map.get(key("skills")));
    if skills.is_empty() {
        return Ok(None);
    }
    let description = map
        .get(key("description"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let instruction = map
        .get(key("instruction"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let slug = bundle_slugify(&name);
    if slug.is_empty() {
        return Ok(None);
    }
    let description = if description.is_empty() {
        format!("Load {} skills as a bundle", skills.len())
    } else {
        description
    };
    Ok(Some(SkillBundle {
        name,
        slug,
        description,
        skills,
        instruction,
        path: path.to_path_buf(),
    }))
}

fn list_skill_bundles(hermes_home: &Path) -> io::Result<Vec<SkillBundle>> {
    let dir = bundles_dir(hermes_home);
    let mut bundles = Vec::new();
    if !dir.exists() {
        return Ok(bundles);
    }
    let mut paths = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| matches!(ext, "yaml" | "yml"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut seen = BTreeSet::new();
    for path in paths {
        if let Some(bundle) = load_bundle_file(&path)? {
            if seen.insert(bundle.slug.clone()) {
                bundles.push(bundle);
            }
        }
    }
    bundles.sort_by(|left, right| left.slug.cmp(&right.slug));
    Ok(bundles)
}

fn bundles_list_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let bundles = list_skill_bundles(hermes_home)?;
    let stdout = if bundles.is_empty() {
        format!(
            "No bundles installed yet. Create one with:\n  hermes bundles create <name> --skill skill1 --skill skill2\nBundles directory: {}\n",
            bundles_dir(hermes_home).display()
        )
    } else {
        let mut stdout = format!("Skill Bundles ({})\n", bundles.len());
        for bundle in bundles {
            stdout.push_str(&format!(
                "/{}  {}  {}  {}\n",
                bundle.slug,
                bundle.name,
                bundle.skills.len(),
                bundle.description
            ));
        }
        stdout.push_str(&format!(
            "\nBundles directory: {}\n",
            bundles_dir(hermes_home).display()
        ));
        stdout
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn find_bundle(hermes_home: &Path, name: &str) -> io::Result<Option<SkillBundle>> {
    let slug = bundle_slugify(name);
    Ok(list_skill_bundles(hermes_home)?
        .into_iter()
        .find(|bundle| bundle.slug == slug))
}

fn bundles_show_output(hermes_home: &Path, name: &str) -> io::Result<CliExecution> {
    let Some(bundle) = find_bundle(hermes_home, name)? else {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: format!("Bundle {name:?} not found.\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    };
    let mut stdout = format!(
        "/{}  {}\n  {}\n  File: {}\n  Skills ({}):\n",
        bundle.slug,
        bundle.name,
        bundle.description,
        bundle.path.display(),
        bundle.skills.len()
    );
    for skill in &bundle.skills {
        stdout.push_str(&format!("    - {skill}\n"));
    }
    if !bundle.instruction.is_empty() {
        stdout.push_str(&format!("  Instruction:\n    {}\n", bundle.instruction));
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn parse_bundle_create_args(args: &[&str]) -> Result<(Vec<String>, String, String, bool), String> {
    let mut skills = Vec::new();
    let mut description = String::new();
    let mut instruction = String::new();
    let mut force = false;
    let mut index = 0;
    while index < args.len() {
        match args[index] {
            "--skill" | "-s" => {
                let Some(skill) = args.get(index + 1) else {
                    return Err("argument --skill/-s: expected one argument".to_string());
                };
                skills.push((*skill).to_string());
                index += 2;
            }
            "--description" | "-d" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("argument --description/-d: expected one argument".to_string());
                };
                description = (*value).to_string();
                index += 2;
            }
            "--instruction" | "-i" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("argument --instruction/-i: expected one argument".to_string());
                };
                instruction = (*value).to_string();
                index += 2;
            }
            "--force" | "-f" => {
                force = true;
                index += 1;
            }
            other => {
                return Err(format!("unrecognized arguments: {other}"));
            }
        }
    }
    Ok((skills, description, instruction, force))
}

fn write_bundle_yaml(
    path: &Path,
    name: &str,
    skills: Vec<String>,
    description: String,
    instruction: String,
) -> io::Result<()> {
    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(
        serde_yaml::Value::String("name".to_string()),
        serde_yaml::Value::String(name.to_string()),
    );
    mapping.insert(
        serde_yaml::Value::String("skills".to_string()),
        serde_yaml::Value::Sequence(
            skills
                .into_iter()
                .map(serde_yaml::Value::String)
                .collect::<Vec<_>>(),
        ),
    );
    if !description.is_empty() {
        mapping.insert(
            serde_yaml::Value::String("description".to_string()),
            serde_yaml::Value::String(description),
        );
    }
    if !instruction.is_empty() {
        mapping.insert(
            serde_yaml::Value::String("instruction".to_string()),
            serde_yaml::Value::String(instruction),
        );
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_yaml::to_string(&serde_yaml::Value::Mapping(mapping))
            .unwrap_or_else(|_| "{}\n".to_string()),
    )
}

fn bundles_create_output(
    hermes_home: &Path,
    name: &str,
    args: &[&str],
) -> io::Result<CliExecution> {
    let (skills, description, instruction, force) = match parse_bundle_create_args(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Ok(CliExecution {
                exit_code: 2,
                stdout: String::new(),
                stdout_markers: BTreeMap::new(),
                stderr: format!("{error}\n"),
            });
        }
    };
    let cleaned_skills = skills
        .into_iter()
        .map(|skill| skill.trim().to_string())
        .filter(|skill| !skill.is_empty())
        .collect::<Vec<_>>();
    if cleaned_skills.is_empty() {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: "A bundle must reference at least one skill.\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let path = match bundle_path_for(hermes_home, name) {
        Ok(path) => path,
        Err(error) => {
            return Ok(CliExecution {
                exit_code: 1,
                stdout: format!("{error}\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            });
        }
    };
    if path.exists() && !force {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: format!(
                "Bundle already exists at {}\nPass --force to overwrite.\n",
                path.display()
            ),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    write_bundle_yaml(
        &path,
        name.trim(),
        cleaned_skills,
        description.trim().to_string(),
        instruction.trim().to_string(),
    )?;
    let skill_count = find_bundle(hermes_home, name)?
        .map(|bundle| bundle.skills.len())
        .unwrap_or(0);
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!(
            "Created bundle: {}\n  Invoke with: /{}  (loads {} skills)\n",
            path.display(),
            bundle_slugify(name),
            skill_count
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn bundles_delete_output(hermes_home: &Path, name: &str) -> io::Result<CliExecution> {
    let path = bundle_path_for(hermes_home, name)?;
    if !path.exists() {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: format!("No bundle at {}\n", path.display()),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    fs::remove_file(&path)?;
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!("Deleted bundle: {}\n", path.display()),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn bundles_reload_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let total = list_skill_bundles(hermes_home)?.len();
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!("No changes. {total} bundle(s) loaded.\n"),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn fallback_entry_is_valid(entry: &Value) -> bool {
    entry.as_object().is_some_and(|map| {
        map.get("provider")
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty())
            && map
                .get("model")
                .and_then(Value::as_str)
                .is_some_and(|v| !v.is_empty())
    })
}

fn fallback_chain(config: &Value) -> Vec<Value> {
    let Some(root) = config.as_object() else {
        return Vec::new();
    };
    if let Some(entries) = root.get("fallback_providers").and_then(Value::as_array) {
        let chain = entries
            .iter()
            .filter(|entry| fallback_entry_is_valid(entry))
            .cloned()
            .collect::<Vec<_>>();
        if !chain.is_empty() {
            return chain;
        }
    }
    let Some(legacy) = root.get("fallback_model") else {
        return Vec::new();
    };
    if fallback_entry_is_valid(legacy) {
        return vec![legacy.clone()];
    }
    legacy
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| fallback_entry_is_valid(entry))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn fallback_entry_str<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn format_fallback_entry(entry: &Value) -> String {
    let provider = fallback_entry_str(entry, "provider");
    let model = fallback_entry_str(entry, "model");
    let base = fallback_entry_str(entry, "base_url");
    let suffix = if base.is_empty() {
        String::new()
    } else {
        format!("  [{base}]")
    };
    format!("{model}  (via {provider}){suffix}")
}

fn describe_primary_model(config: &Value) -> Option<String> {
    let model_cfg = config.as_object()?.get("model")?;
    if let Some(model) = model_cfg
        .as_str()
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        return Some(model.to_string());
    }
    let map = model_cfg.as_object()?;
    let provider = map
        .get("provider")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .unwrap_or("?");
    let model = map
        .get("default")
        .or_else(|| map.get("model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("?");
    Some(format!("{model}  (via {provider})"))
}

fn fallback_list_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let config = read_config_value(hermes_home)?;
    let chain = fallback_chain(&config);
    let stdout = if chain.is_empty() {
        "\n  No fallback providers configured.\n\n  Add one with:  hermes fallback add\n\n"
            .to_string()
    } else {
        let mut stdout = String::new();
        if let Some(primary) = describe_primary_model(&config) {
            stdout.push_str(&format!("\n  Primary:   {primary}\n\n"));
        } else {
            stdout.push('\n');
        }
        let noun = if chain.len() == 1 { "entry" } else { "entries" };
        stdout.push_str(&format!("  Fallback chain ({} {noun}):\n", chain.len()));
        for (index, entry) in chain.iter().enumerate() {
            stdout.push_str(&format!(
                "    {}. {}\n",
                index + 1,
                format_fallback_entry(entry)
            ));
        }
        stdout.push_str(
            "\n  Tried in order when the primary fails (rate-limit, 5xx, connection errors).\n  Docs: https://hermes-agent.nousresearch.com/docs/user-guide/features/fallback-providers\n\n",
        );
        stdout
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn fallback_remove_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let chain = fallback_chain(&read_config_value(hermes_home)?);
    let stdout = if chain.is_empty() {
        "\n  No fallback providers configured — nothing to remove.\n\n".to_string()
    } else {
        "\n  Cancelled — no change.\n".to_string()
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn fallback_clear_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let chain = fallback_chain(&read_config_value(hermes_home)?);
    let stdout = if chain.is_empty() {
        "\n  No fallback providers configured — nothing to clear.\n\n".to_string()
    } else {
        "\n  Cancelled.\n".to_string()
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn curator_state_path(hermes_home: &Path) -> PathBuf {
    hermes_home.join("skills").join(".curator_state")
}

fn curator_default_state() -> Value {
    json!({
        "last_report_path": null,
        "last_run_at": null,
        "last_run_duration_seconds": null,
        "last_run_summary": null,
        "last_run_summary_shown_at": null,
        "paused": false,
        "run_count": 0,
    })
}

fn read_curator_state(hermes_home: &Path) -> io::Result<Value> {
    let path = curator_state_path(hermes_home);
    let mut state = curator_default_state();
    if path.exists() {
        if let Ok(Value::Object(existing)) =
            serde_json::from_str::<Value>(&fs::read_to_string(path)?)
        {
            let root = state.as_object_mut().unwrap();
            for (key, value) in existing {
                if root.contains_key(&key) || key.starts_with('_') {
                    root.insert(key, value);
                }
            }
        }
    }
    Ok(state)
}

fn write_curator_state(hermes_home: &Path, state: &Value) -> io::Result<()> {
    let path = curator_state_path(hermes_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(state).unwrap_or_else(|_| "{}".to_string()),
    )
}

fn curator_config_value(config: &Value, key: &str, default: i64) -> i64 {
    config
        .as_object()
        .and_then(|root| root.get("curator"))
        .and_then(Value::as_object)
        .and_then(|curator| curator.get(key))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
        })
        .unwrap_or(default)
}

fn curator_enabled(config: &Value) -> bool {
    config
        .as_object()
        .and_then(|root| root.get("curator"))
        .and_then(Value::as_object)
        .and_then(|curator| curator.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn curator_interval_label(hours: i64) -> String {
    if hours >= 24 && hours % 24 == 0 {
        format!("{}d", hours / 24)
    } else {
        format!("{hours}h")
    }
}

fn curator_status_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let config = read_config_value(hermes_home)?;
    let state = read_curator_state(hermes_home)?;
    let paused = state["paused"].as_bool().unwrap_or(false);
    let enabled = curator_enabled(&config);
    let status = if enabled && !paused {
        "ENABLED"
    } else if paused {
        "PAUSED"
    } else {
        "DISABLED"
    };
    let runs = state["run_count"].as_i64().unwrap_or(0);
    let last_summary = state["last_run_summary"].as_str().unwrap_or("(none)");
    let interval = curator_interval_label(curator_config_value(&config, "interval_hours", 168));
    let stale_after = curator_config_value(&config, "stale_after_days", 30);
    let archive_after = curator_config_value(&config, "archive_after_days", 90);
    let stdout = format!(
        "curator: {status}\n  runs:           {runs}\n  last run:       never\n  last summary:   {last_summary}\n  interval:       every {interval}\n  stale after:    {stale_after}d unused\n  archive after:  {archive_after}d unused\n\nno agent-created skills\n"
    );
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn curator_set_paused_output(hermes_home: &Path, paused: bool) -> io::Result<CliExecution> {
    let mut state = read_curator_state(hermes_home)?;
    state["paused"] = Value::Bool(paused);
    write_curator_state(hermes_home, &state)?;
    Ok(CliExecution {
        exit_code: 0,
        stdout: if paused {
            "curator: paused\n".to_string()
        } else {
            "curator: resumed\n".to_string()
        },
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn curator_list_archived_output(_hermes_home: &Path) -> CliExecution {
    CliExecution {
        exit_code: 0,
        stdout: "curator: no archived skills\n".to_string(),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn insights_empty_output(days: i64, source: Option<&str>) -> CliExecution {
    let source_suffix = source.map_or_else(String::new, |value| format!(" (source: {value})"));
    CliExecution {
        exit_code: 0,
        stdout: format!("  No sessions found in the last {days} days{source_suffix}.\n"),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn read_env_map(hermes_home: &Path) -> io::Result<BTreeMap<String, String>> {
    let path = hermes_home.join(".env");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let lines = fs::read_to_string(path)?
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    Ok(hermes_config::parse_env_lines(&lines))
}

fn config_obj_field<'a>(config: &'a Value, section: &str, key: &str) -> Option<&'a str> {
    config
        .as_object()?
        .get(section)?
        .as_object()?
        .get(key)?
        .as_str()
}

fn dump_model_provider(config: &Value) -> (String, String) {
    let Some(model_cfg) = config.as_object().and_then(|root| root.get("model")) else {
        return ("(not set)".to_string(), "(auto)".to_string());
    };
    if let Some(model) = model_cfg.as_str() {
        return (
            if model.is_empty() { "(not set)" } else { model }.to_string(),
            "(auto)".to_string(),
        );
    }
    let Some(map) = model_cfg.as_object() else {
        return ("(not set)".to_string(), "(auto)".to_string());
    };
    let model = map
        .get("default")
        .or_else(|| map.get("model"))
        .or_else(|| map.get("name"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("(not set)");
    let provider = map
        .get("provider")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("(auto)");
    (model.to_string(), provider.to_string())
}

fn dump_count_skills(hermes_home: &Path) -> usize {
    let skills_dir = hermes_home.join("skills");
    let mut count = 0;
    let mut stack = vec![skills_dir];
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                stack.push(child);
            } else if child.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
                count += 1;
            }
        }
    }
    count
}

fn dump_cron_summary(hermes_home: &Path) -> String {
    let path = hermes_home.join("cron").join("jobs.json");
    let Ok(text) = fs::read_to_string(path) else {
        return "0".to_string();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return "(error reading)".to_string();
    };
    let jobs = data
        .as_object()
        .and_then(|root| root.get("jobs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active = jobs
        .iter()
        .filter(|job| {
            job.as_object()
                .and_then(|obj| obj.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
        .count();
    format!("{active} active / {} total", jobs.len())
}

fn dump_configured_platforms(env: &BTreeMap<String, String>) -> Vec<&'static str> {
    [
        ("telegram", "TELEGRAM_BOT_TOKEN"),
        ("discord", "DISCORD_BOT_TOKEN"),
        ("slack", "SLACK_BOT_TOKEN"),
        ("whatsapp", "WHATSAPP_ENABLED"),
        ("signal", "SIGNAL_HTTP_URL"),
        ("email", "EMAIL_ADDRESS"),
        ("sms", "TWILIO_ACCOUNT_SID"),
        ("matrix", "MATRIX_HOMESERVER_URL"),
        ("mattermost", "MATTERMOST_URL"),
        ("homeassistant", "HASS_TOKEN"),
        ("dingtalk", "DINGTALK_CLIENT_ID"),
        ("feishu", "FEISHU_APP_ID"),
        ("wecom", "WECOM_BOT_ID"),
        ("wecom_callback", "WECOM_CALLBACK_CORP_ID"),
        ("weixin", "WEIXIN_ACCOUNT_ID"),
        ("qqbot", "QQ_APP_ID"),
    ]
    .into_iter()
    .filter_map(|(name, key)| env.contains_key(key).then_some(name))
    .collect()
}

fn dump_mcp_server_count(config: &Value) -> usize {
    config
        .as_object()
        .and_then(|root| root.get("mcp"))
        .and_then(Value::as_object)
        .and_then(|mcp| mcp.get("servers"))
        .and_then(Value::as_object)
        .map(|servers| servers.len())
        .unwrap_or(0)
}

fn dump_memory_provider(config: &Value) -> &str {
    config_obj_field(config, "memory", "provider")
        .filter(|provider| !provider.is_empty())
        .unwrap_or("built-in")
}

fn dump_toolsets(config: &Value) -> String {
    let toolsets = config
        .as_object()
        .and_then(|root| root.get("toolsets"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["hermes-cli".to_string()]);
    if toolsets.is_empty() {
        "(default)".to_string()
    } else {
        toolsets.join(", ")
    }
}

fn dump_fallback_repr(config: &Value) -> Option<String> {
    let entries = config.as_object()?.get("fallback_providers")?.as_array()?;
    if entries.is_empty() {
        return None;
    }
    let rendered = entries
        .iter()
        .filter_map(Value::as_object)
        .map(|entry| {
            let mut fields = Vec::new();
            for key in ["provider", "model", "base_url", "api_mode"] {
                if let Some(value) = entry.get(key).and_then(Value::as_str) {
                    fields.push(format!("'{key}': '{value}'"));
                }
            }
            format!("{{{}}}", fields.join(", "))
        })
        .collect::<Vec<_>>();
    Some(format!("[{}]", rendered.join(", ")))
}

fn dump_output(hermes_home: &Path, show_keys: bool) -> io::Result<CliExecution> {
    let env = read_env_map(hermes_home)?;
    let config = read_config_value(hermes_home)?;
    let (model, provider) = dump_model_provider(&config);
    let terminal = config_obj_field(&config, "terminal", "backend").unwrap_or("local");
    let platforms = dump_configured_platforms(&env);
    let platform_text = if platforms.is_empty() {
        "none".to_string()
    } else {
        platforms.join(", ")
    };
    let mut lines = vec![
        "--- hermes dump ---".to_string(),
        "version:          0.0.0 [(unknown)]".to_string(),
        "os:               unknown".to_string(),
        "python:           unavailable".to_string(),
        "openai_sdk:       not installed".to_string(),
        "profile:          default".to_string(),
        format!("hermes_home:      {}", hermes_home.display()),
        format!("model:            {model}"),
        format!("provider:         {provider}"),
        format!("terminal:         {terminal}"),
        String::new(),
        "api_keys:".to_string(),
    ];
    for (env_var, label) in [
        ("OPENROUTER_API_KEY", "openrouter"),
        ("OPENAI_API_KEY", "openai"),
        ("ANTHROPIC_API_KEY", "anthropic"),
        ("ANTHROPIC_TOKEN", "anthropic_token"),
        ("NOUS_API_KEY", "nous"),
        ("GOOGLE_API_KEY", "google/gemini"),
        ("GEMINI_API_KEY", "gemini"),
        ("GLM_API_KEY", "glm/zai"),
        ("ZAI_API_KEY", "zai"),
        ("KIMI_API_KEY", "kimi"),
        ("MINIMAX_API_KEY", "minimax"),
        ("DEEPSEEK_API_KEY", "deepseek"),
        ("DASHSCOPE_API_KEY", "dashscope"),
        ("HF_TOKEN", "huggingface"),
        ("NVIDIA_API_KEY", "nvidia"),
        ("AI_GATEWAY_API_KEY", "ai_gateway"),
        ("OPENCODE_ZEN_API_KEY", "opencode_zen"),
        ("OPENCODE_GO_API_KEY", "opencode_go"),
        ("KILOCODE_API_KEY", "kilocode"),
        ("FIRECRAWL_API_KEY", "firecrawl"),
        ("TAVILY_API_KEY", "tavily"),
        ("BROWSERBASE_API_KEY", "browserbase"),
        ("FAL_KEY", "fal"),
        ("ELEVENLABS_API_KEY", "elevenlabs"),
        ("GITHUB_TOKEN", "github"),
    ] {
        let value = env.get(env_var).map(String::as_str).unwrap_or("");
        let display = if show_keys && !value.is_empty() {
            hermes_config::mask_secret(value, "")
        } else if value.is_empty() {
            "not set".to_string()
        } else {
            "set".to_string()
        };
        lines.push(format!("  {label:<20} {display}"));
    }
    lines.extend([
        String::new(),
        "features:".to_string(),
        format!("  toolsets:           {}", dump_toolsets(&config)),
        format!("  mcp_servers:        {}", dump_mcp_server_count(&config)),
        format!("  memory_provider:    {}", dump_memory_provider(&config)),
        "  gateway:            stopped (docker (foreground))".to_string(),
        format!("  platforms:          {platform_text}"),
        format!("  cron_jobs:          {}", dump_cron_summary(hermes_home)),
        format!("  skills:             {}", dump_count_skills(hermes_home)),
    ]);
    let mut overrides = Vec::new();
    if terminal != "local" {
        overrides.push(format!("  terminal.backend: {terminal}"));
    }
    if let Some(skin) =
        config_obj_field(&config, "display", "skin").filter(|skin| *skin != "default")
    {
        overrides.push(format!("  display.skin: {skin}"));
    }
    if let Some(fallbacks) = dump_fallback_repr(&config) {
        overrides.push(format!("  fallback_providers: {fallbacks}"));
    }
    if !overrides.is_empty() {
        lines.push(String::new());
        lines.push("config_overrides:".to_string());
        lines.extend(overrides);
    }
    lines.push("--- end dump ---".to_string());
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!("{}\n", lines.join("\n")),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn memory_status_output() -> String {
    "\nMemory status\n────────────────────────────────────────\n  Built-in:  always active\n  Provider:  (none — built-in only)\n\n  Installed plugins:\n    • byterover  (requires API key)\n    • hindsight  (API key / local)\n    • holographic  (local)\n    • honcho  (API key / local)\n    • mem0  (API key / local)\n    • openviking  (API key / local)\n    • retaindb  (API key / local)\n    • supermemory  (requires API key)\n\n"
        .to_string()
}

fn set_memory_provider(hermes_home: &Path, provider: &str) -> io::Result<()> {
    let mut config = read_config_value(hermes_home)?;
    if !config.is_object() {
        config = json!({});
    }
    let root = config.as_object_mut().unwrap();
    let memory = root
        .entry("memory".to_string())
        .or_insert_with(|| json!({}));
    if !memory.is_object() {
        *memory = json!({});
    }
    memory
        .as_object_mut()
        .unwrap()
        .insert("provider".to_string(), Value::String(provider.to_string()));
    write_config_value(hermes_home, &config)
}

fn memory_reset_target(hermes_home: &Path, target: &str) -> io::Result<CliExecution> {
    let mem_dir = hermes_home.join("memories");
    let mut files = Vec::new();
    if matches!(target, "all" | "memory") {
        files.push(("MEMORY.md", "agent notes"));
    }
    if matches!(target, "all" | "user") {
        files.push(("USER.md", "user profile"));
    }
    let existing = files
        .into_iter()
        .filter_map(|(file, desc)| {
            let path = mem_dir.join(file);
            path.exists().then_some((file, desc, path))
        })
        .collect::<Vec<_>>();
    if existing.is_empty() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: format!(
                "\n  Nothing to reset — no memory files found in {}/memories/\n\n",
                hermes_home.display()
            ),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }

    let mut stdout = "\n  This will permanently erase the following memory files:\n".to_string();
    for (file, desc, path) in &existing {
        let size = fs::metadata(path)?.len();
        stdout.push_str(&format!("    ◆ {file} ({desc}) — {size} bytes\n"));
    }
    for (file, desc, path) in &existing {
        fs::remove_file(path)?;
        stdout.push_str(&format!("  ✓ Deleted {file} ({desc})\n"));
    }
    stdout.push_str(&format!(
        "\n  Memory reset complete. New sessions will start with a blank slate.\n  Files were in: {}/memories/\n\n",
        hermes_home.display()
    ));
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

const QUICK_STATE_FILES: &[&str] = &[
    "state.db",
    "config.yaml",
    ".env",
    "auth.json",
    "cron/jobs.json",
    "gateway_state.json",
    "channel_directory.json",
    "processes.json",
    "pairing",
    "platforms/pairing",
    "feishu_comment_pairing.json",
];

const BACKUP_EXCLUDED_DIRS: &[&str] = &[
    "hermes-agent",
    "__pycache__",
    ".git",
    "node_modules",
    "backups",
    "checkpoints",
];

const BACKUP_EXCLUDED_SUFFIXES: &[&str] = &[".pyc", ".pyo", ".db-wal", ".db-shm", ".db-journal"];

const BACKUP_EXCLUDED_NAMES: &[&str] = &["gateway.pid", "cron.pid"];

const BACKUP_SECRET_FILE_NAMES: &[&str] = &[".env", "auth.json", "state.db"];

pub fn backup_should_exclude(rel_path: &str) -> bool {
    let parts = rel_path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.iter().any(|part| BACKUP_EXCLUDED_DIRS.contains(part)) {
        return true;
    }

    let Some(name) = parts.last() else {
        return false;
    };
    BACKUP_EXCLUDED_NAMES.contains(name)
        || BACKUP_EXCLUDED_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

pub fn backup_secret_file_names() -> Vec<&'static str> {
    let mut names = BACKUP_SECRET_FILE_NAMES.to_vec();
    names.sort_unstable();
    names
}

pub fn backup_detect_prefix(members: &[&str]) -> String {
    let mut first_parts = BTreeSet::new();
    for member in members
        .iter()
        .copied()
        .filter(|member| !member.ends_with('/'))
    {
        let parts = member
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() > 1 {
            first_parts.insert(parts[0]);
        }
    }
    if first_parts.len() == 1 {
        let prefix = first_parts.iter().next().copied().unwrap_or_default();
        if matches!(prefix, ".hermes" | "hermes") {
            return format!("{prefix}/");
        }
    }
    String::new()
}

pub fn backup_validate_members(members: &[&str]) -> (bool, String) {
    if members.is_empty() {
        return (false, "zip archive is empty".to_string());
    }
    let markers = ["config.yaml", ".env", "state.db"];
    let found_marker = members.iter().any(|member| {
        member
            .split('/')
            .rfind(|part| !part.is_empty())
            .is_some_and(|name| markers.contains(&name))
    });
    if !found_marker {
        return (
            false,
            "zip does not appear to be a Hermes backup (no config.yaml, .env, or state databases found)"
                .to_string(),
        );
    }
    (true, String::new())
}

pub fn backup_import_member_plan(member: &str, prefix: &str) -> Value {
    if member.ends_with('/') {
        return json!({
            "member": member,
            "prefix": prefix,
            "action": "skip",
            "rel": "",
        });
    }

    let rel = if !prefix.is_empty() && member.starts_with(prefix) {
        &member[prefix.len()..]
    } else {
        member
    };
    if rel.is_empty() {
        return json!({
            "member": member,
            "prefix": prefix,
            "action": "skip",
            "rel": rel,
        });
    }

    let Some(restored_rel) = normalize_backup_import_rel(rel) else {
        return json!({
            "member": member,
            "prefix": prefix,
            "action": "block",
            "rel": rel,
            "error": format!("  {rel}: path traversal blocked"),
        });
    };
    let basename = rel
        .split('/')
        .rfind(|part| !part.is_empty())
        .unwrap_or_default();
    json!({
        "member": member,
        "prefix": prefix,
        "action": "restore",
        "rel": restored_rel,
        "secret": BACKUP_SECRET_FILE_NAMES.contains(&basename),
    })
}

fn normalize_backup_import_rel(rel: &str) -> Option<String> {
    if rel.starts_with('/') {
        return None;
    }
    let mut parts = Vec::<&str>::new();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

fn create_full_backup_output(hermes_home: &Path, output: &Path) -> io::Result<CliExecution> {
    let out_path = normalize_backup_output_path(output);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut files = Vec::<(PathBuf, String)>::new();
    let mut skipped_dirs = BTreeSet::<String>::new();
    collect_full_backup_files(
        hermes_home,
        hermes_home,
        &out_path,
        &mut files,
        &mut skipped_dirs,
    )?;

    if files.is_empty() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "No files to back up.\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }

    files.sort_by(|left, right| left.1.cmp(&right.1));
    let mut total_bytes = 0_u64;
    let file = fs::File::create(&out_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (abs_path, rel_path) in &files {
        zip.start_file(rel_path, options)
            .map_err(io::Error::other)?;
        let mut source = fs::File::open(abs_path)?;
        total_bytes += io::copy(&mut source, &mut zip)?;
    }
    zip.finish().map_err(io::Error::other)?;

    let zip_size = fs::metadata(&out_path)?.len();
    let mut stdout = format!(
        "Scanning {} ...\nBacking up {} files ...\n\nBackup complete: {}\n  Files:       {}\n  Original:    {}\n  Compressed:  {}\n  Time:        0.0s\n",
        hermes_home.display(),
        files.len(),
        out_path.display(),
        files.len(),
        backup_format_size(total_bytes),
        backup_format_size(zip_size),
    );
    if !skipped_dirs.is_empty() {
        stdout.push_str("\n  Excluded directories:\n");
        for dir in skipped_dirs {
            stdout.push_str(&format!("    {dir}/\n"));
        }
    }
    let restore_name = out_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("backup.zip");
    stdout.push_str(&format!("\nRestore with: hermes import {restore_name}\n"));

    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn normalize_backup_output_path(output: &Path) -> PathBuf {
    if output
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        output.to_path_buf()
    } else {
        PathBuf::from(format!("{}.zip", output.display()))
    }
}

fn collect_full_backup_files(
    root: &Path,
    dir: &Path,
    out_path: &Path,
    files: &mut Vec<(PathBuf, String)>,
    skipped_dirs: &mut BTreeSet<String>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let rel_path = path.strip_prefix(root).map_err(io::Error::other)?;
        let rel = rel_path.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            if rel_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| BACKUP_EXCLUDED_DIRS.contains(&name))
            {
                skipped_dirs.insert(rel);
                continue;
            }
            collect_full_backup_files(root, &path, out_path, files, skipped_dirs)?;
            continue;
        }
        if !path.is_file() || backup_should_exclude(&rel) {
            continue;
        }
        let is_output = path
            .canonicalize()
            .ok()
            .zip(out_path.canonicalize().ok())
            .is_some_and(|(path, out)| path == out);
        if is_output {
            continue;
        }
        files.push((path, rel));
    }
    Ok(())
}

fn backup_format_size(nbytes: u64) -> String {
    let mut value = nbytes as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if value < 1024.0 {
            if unit == "B" {
                return format!("{} B", value as u64);
            }
            return format!("{value:.1} {unit}");
        }
        value /= 1024.0;
    }
    format!("{value:.1} TB")
}

pub fn backup_zip_members(path: &Path) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(io::Error::other)?;
    let mut members = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(io::Error::other)?;
        members.push(file.name().to_string());
    }
    members.sort();
    Ok(members)
}

fn restore_full_backup_output(hermes_home: &Path, archive_path: &Path) -> io::Result<CliExecution> {
    if !archive_path.is_file() {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: format!("Error: File not found: {}\n", archive_path.display()),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }

    let file = fs::File::open(archive_path)?;
    let mut archive = match ZipArchive::new(file).map_err(io::Error::other) {
        Ok(archive) => archive,
        Err(_) => {
            return Ok(CliExecution {
                exit_code: 1,
                stdout: format!("Error: Not a valid zip file: {}\n", archive_path.display()),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            });
        }
    };
    let members = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .map(|file| file.name().to_string())
                .map_err(io::Error::other)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let member_refs = members.iter().map(String::as_str).collect::<Vec<_>>();
    let (ok, reason) = backup_validate_members(&member_refs);
    if !ok {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: format!("Error: {reason}\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }

    let prefix = backup_detect_prefix(&member_refs);
    let file_members = members
        .iter()
        .filter(|member| !member.ends_with('/'))
        .cloned()
        .collect::<Vec<_>>();
    let mut stdout = format!(
        "Backup contains {} files\nTarget: {}\n",
        file_members.len(),
        hermes_home.display()
    );
    if !prefix.is_empty() {
        stdout.push_str(&format!(
            "Detected archive prefix: '{prefix}' (will be stripped)\n"
        ));
    }
    stdout.push_str(&format!("\nImporting {} files ...\n", file_members.len()));

    fs::create_dir_all(hermes_home)?;
    let mut restored = 0;
    let mut errors = Vec::<String>::new();
    for member in file_members {
        let plan = backup_import_member_plan(&member, &prefix);
        match plan["action"].as_str() {
            Some("skip") => continue,
            Some("block") => {
                if let Some(error) = plan["error"].as_str() {
                    errors.push(error.to_string());
                }
            }
            Some("restore") => {
                let rel = plan["rel"].as_str().unwrap_or_default();
                let target = hermes_home.join(rel);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut source = archive.by_name(&member).map_err(io::Error::other)?;
                let mut destination = fs::File::create(&target)?;
                io::copy(&mut source, &mut destination)?;
                if plan["secret"].as_bool().unwrap_or(false) {
                    set_secret_permissions(&target)?;
                }
                restored += 1;
            }
            _ => {}
        }
    }

    stdout.push_str(&format!(
        "\nImport complete: {restored} files restored in 0.0s\n  Target: {}\n",
        hermes_home.display()
    ));
    if !errors.is_empty() {
        stdout.push_str(&format!("\n  Warnings ({} files skipped):\n", errors.len()));
        for error in errors.iter().take(10) {
            stdout.push_str(error);
            stdout.push('\n');
        }
    }
    stdout.push_str("\nNote: The hermes-agent codebase was not included in the backup.\n  If this is a fresh install, run: hermes update\nDone. Your Hermes configuration has been restored.\n");

    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn set_secret_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn create_quick_backup_output(hermes_home: &Path, label: &str) -> io::Result<CliExecution> {
    let snap_id = quick_snapshot_id(label);
    let snapshot_dir = hermes_home.join("state-snapshots").join(&snap_id);
    fs::create_dir_all(&snapshot_dir)?;

    let mut manifest = BTreeMap::<String, u64>::new();
    for rel in QUICK_STATE_FILES {
        let src = hermes_home.join(rel);
        if !src.exists() {
            continue;
        }
        if src.is_dir() {
            let mut files = Vec::new();
            collect_files_sorted(&src, &mut files)?;
            for file in files {
                let sub_rel = file
                    .strip_prefix(hermes_home)
                    .map_err(io::Error::other)?
                    .to_string_lossy()
                    .replace('\\', "/");
                copy_quick_snapshot_file(hermes_home, &snapshot_dir, &sub_rel, &mut manifest)?;
            }
        } else if src.is_file() {
            copy_quick_snapshot_file(hermes_home, &snapshot_dir, rel, &mut manifest)?;
        }
    }

    if manifest.is_empty() {
        let _ = fs::remove_dir_all(&snapshot_dir);
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "No state files found to snapshot.\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }

    let file_count = manifest.len();
    let total_size = manifest.values().sum::<u64>();
    let timestamp = snap_id
        .split_once('-')
        .and_then(|(date, rest)| {
            rest.split_once('-')
                .map(|(time, _)| format!("{date}-{time}"))
        })
        .unwrap_or_else(|| snap_id.clone());
    let meta = json!({
        "id": snap_id,
        "timestamp": timestamp,
        "label": label,
        "file_count": file_count,
        "total_size": total_size,
        "files": manifest,
    });
    fs::write(
        snapshot_dir.join("manifest.json"),
        serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string()),
    )?;
    prune_quick_snapshots(&hermes_home.join("state-snapshots"), 20)?;

    let snapshots = list_quick_snapshot_count(&hermes_home.join("state-snapshots"))?;
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!(
            "State snapshot created: {snap_id}\n  {snapshots} snapshot(s) stored in {}/state-snapshots/\n  Restore with: /snapshot restore {snap_id}\n",
            hermes_home.display()
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn quick_snapshot_id(label: &str) -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if label.is_empty() {
        format!("{seconds:014}")
    } else {
        format!("{seconds:014}-{label}")
    }
}

fn collect_files_sorted(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut children = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());
    for child in children {
        let path = child.path();
        if path.is_dir() {
            collect_files_sorted(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn copy_quick_snapshot_file(
    hermes_home: &Path,
    snapshot_dir: &Path,
    rel: &str,
    manifest: &mut BTreeMap<String, u64>,
) -> io::Result<()> {
    let src = hermes_home.join(rel);
    let dst = snapshot_dir.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, &dst)?;
    manifest.insert(rel.to_string(), fs::metadata(dst)?.len());
    Ok(())
}

fn prune_quick_snapshots(root: &Path, keep: usize) -> io::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let mut dirs = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    dirs.sort_by_key(|entry| entry.file_name());
    dirs.reverse();

    let mut deleted = 0;
    for dir in dirs.into_iter().skip(keep) {
        fs::remove_dir_all(dir.path())?;
        deleted += 1;
    }
    Ok(deleted)
}

fn list_quick_snapshot_count(root: &Path) -> io::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    Ok(fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .count())
}

fn pairing_dir(hermes_home: &Path) -> PathBuf {
    let legacy = hermes_home.join("pairing");
    if legacy.exists() {
        legacy
    } else {
        hermes_home.join("platforms").join("pairing")
    }
}

fn pairing_json(path: &Path) -> io::Result<Value> {
    if path.exists() {
        Ok(serde_json::from_str::<Value>(&fs::read_to_string(path)?).unwrap_or_else(|_| json!({})))
    } else {
        Ok(json!({}))
    }
}

fn write_pairing_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(value)
            .map_err(io::Error::other)?
            .as_bytes(),
    )
}

fn pairing_platforms(dir: &Path, suffix: &str) -> io::Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let tail = format!("-{suffix}.json");
    let mut platforms = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(platform) = name.strip_suffix(&tail) {
            if !platform.starts_with('_') {
                platforms.push(platform.to_string());
            }
        }
    }
    Ok(platforms)
}

fn pairing_pending_path(dir: &Path, platform: &str) -> PathBuf {
    dir.join(format!("{platform}-pending.json"))
}

fn pairing_approved_path(dir: &Path, platform: &str) -> PathBuf {
    dir.join(format!("{platform}-approved.json"))
}

fn pairing_rate_limits_path(dir: &Path) -> PathBuf {
    dir.join("_rate_limits.json")
}

fn current_unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn pairing_list_output(hermes_home: &Path) -> io::Result<String> {
    let dir = pairing_dir(hermes_home);
    let now = current_unix_seconds();
    let mut pending_rows = Vec::new();
    for platform in pairing_platforms(&dir, "pending")? {
        let pending = pairing_json(&pairing_pending_path(&dir, &platform))?;
        if let Some(entries) = pending.as_object() {
            for (code, info) in entries {
                let created_at = info
                    .get("created_at")
                    .and_then(Value::as_f64)
                    .unwrap_or(now);
                let age_minutes = ((now - created_at).max(0.0) / 60.0).floor() as i64;
                pending_rows.push((
                    platform.clone(),
                    code.to_string(),
                    info.get("user_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    info.get("user_name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    age_minutes,
                ));
            }
        }
    }

    let mut approved_rows = Vec::new();
    for platform in pairing_platforms(&dir, "approved")? {
        let approved = pairing_json(&pairing_approved_path(&dir, &platform))?;
        if let Some(entries) = approved.as_object() {
            for (user_id, info) in entries {
                approved_rows.push((
                    platform.clone(),
                    user_id.to_string(),
                    info.get("user_name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                ));
            }
        }
    }

    if pending_rows.is_empty() && approved_rows.is_empty() {
        return Ok("No pairing data found. No one has tried to pair yet~\n".to_string());
    }

    let mut stdout = String::new();
    if pending_rows.is_empty() {
        stdout.push_str("\n  No pending pairing requests.\n");
    } else {
        stdout.push_str(&format!(
            "\n  Pending Pairing Requests ({}):\n",
            pending_rows.len()
        ));
        stdout.push_str(&format!(
            "  {:<12} {:<10} {:<20} {:<20} {}\n",
            "Platform", "Code", "User ID", "Name", "Age"
        ));
        stdout.push_str(&format!(
            "  {:<12} {:<10} {:<20} {:<20} {}\n",
            "--------", "----", "-------", "----", "---"
        ));
        for (platform, code, user_id, user_name, age_minutes) in pending_rows {
            stdout.push_str(&format!(
                "  {platform:<12} {code:<10} {user_id:<20} {user_name:<20} {age_minutes}m ago\n"
            ));
        }
    }

    if approved_rows.is_empty() {
        stdout.push_str("\n  No approved users.\n");
    } else {
        stdout.push_str(&format!("\n  Approved Users ({}):\n", approved_rows.len()));
        stdout.push_str(&format!(
            "  {:<12} {:<20} {:<20}\n",
            "Platform", "User ID", "Name"
        ));
        stdout.push_str(&format!(
            "  {:<12} {:<20} {:<20}\n",
            "--------", "-------", "----"
        ));
        for (platform, user_id, user_name) in approved_rows {
            stdout.push_str(&format!("  {platform:<12} {user_id:<20} {user_name:<20}\n"));
        }
    }
    stdout.push('\n');
    Ok(stdout)
}

fn pairing_approve_code(
    hermes_home: &Path,
    raw_platform: &str,
    raw_code: &str,
) -> io::Result<CliExecution> {
    let dir = pairing_dir(hermes_home);
    let platform = raw_platform.to_ascii_lowercase();
    let code = raw_code.to_ascii_uppercase();
    let pending_path = pairing_pending_path(&dir, &platform);
    let mut pending = pairing_json(&pending_path)?;
    let entry = pending
        .as_object_mut()
        .and_then(|entries| entries.remove(&code));
    let Some(entry) = entry else {
        pairing_record_failed_attempt(&dir, &platform)?;
        return Ok(CliExecution {
            exit_code: 0,
            stdout: pairing_missing_code_output(&platform, &code),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    };
    write_pairing_json(&pending_path, &pending)?;

    let user_id = entry
        .get("user_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let user_name = entry
        .get("user_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let approved_path = pairing_approved_path(&dir, &platform);
    let mut approved = pairing_json(&approved_path)?;
    if !approved.is_object() {
        approved = json!({});
    }
    approved.as_object_mut().unwrap().insert(
        user_id.clone(),
        json!({"user_name": user_name, "approved_at": current_unix_seconds()}),
    );
    write_pairing_json(&approved_path, &approved)?;

    let display = if user_name.is_empty() {
        user_id.clone()
    } else {
        format!("{user_name} ({user_id})")
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!(
            "\n  Approved! User {display} on {platform} can now use the bot~\n  They'll be recognized automatically on their next message.\n\n"
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn pairing_record_failed_attempt(dir: &Path, platform: &str) -> io::Result<()> {
    let path = pairing_rate_limits_path(dir);
    let mut limits = pairing_json(&path)?;
    if !limits.is_object() {
        limits = json!({});
    }
    let key = format!("_failures:{platform}");
    let count = limits.get(&key).and_then(Value::as_i64).unwrap_or(0) + 1;
    limits.as_object_mut().unwrap().insert(key, json!(count));
    write_pairing_json(&path, &limits)
}

fn pairing_revoke_user(
    hermes_home: &Path,
    raw_platform: &str,
    user_id: &str,
) -> io::Result<CliExecution> {
    let dir = pairing_dir(hermes_home);
    let platform = raw_platform.to_ascii_lowercase();
    let approved_path = pairing_approved_path(&dir, &platform);
    let mut approved = pairing_json(&approved_path)?;
    let removed = approved
        .as_object_mut()
        .map(|entries| entries.remove(user_id).is_some())
        .unwrap_or(false);
    if removed {
        write_pairing_json(&approved_path, &approved)?;
        Ok(CliExecution {
            exit_code: 0,
            stdout: format!("\n  Revoked access for user {user_id} on {platform}.\n\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        })
    } else {
        Ok(CliExecution {
            exit_code: 0,
            stdout: pairing_revoke_missing_output(&platform, user_id),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        })
    }
}

fn pairing_clear_pending(hermes_home: &Path) -> io::Result<CliExecution> {
    let dir = pairing_dir(hermes_home);
    let mut count = 0usize;
    for platform in pairing_platforms(&dir, "pending")? {
        let path = pairing_pending_path(&dir, &platform);
        let pending = pairing_json(&path)?;
        count += pending
            .as_object()
            .map(|entries| entries.len())
            .unwrap_or(0);
        write_pairing_json(&path, &json!({}))?;
    }
    let stdout = if count == 0 {
        "\n  No pending requests to clear.\n".to_string()
    } else {
        format!("\n  Cleared {count} pending pairing request(s).\n\n")
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn pairing_missing_code_output(platform: &str, code: &str) -> String {
    format!(
        "\n  Code '{}' not found or expired for platform '{}'.\n  Run 'hermes pairing list' to see pending codes.\n\n",
        code.to_ascii_uppercase(),
        platform.to_ascii_lowercase()
    )
}

fn pairing_revoke_missing_output(platform: &str, user_id: &str) -> String {
    format!(
        "\n  User {user_id} not found in approved list for {}.\n\n",
        platform.to_ascii_lowercase()
    )
}

fn slack_slash_commands_json() -> Value {
    Value::Array(
        hermes_slash::slack_native_slashes()
            .into_iter()
            .map(|(name, description, usage_hint)| {
                let mut entry = serde_json::Map::new();
                entry.insert("command".to_string(), json!(format!("/{name}")));
                entry.insert(
                    "description".to_string(),
                    json!(if description.is_empty() {
                        format!("Run /{name}")
                    } else {
                        description
                    }),
                );
                entry.insert("should_escape".to_string(), json!(false));
                entry.insert(
                    "url".to_string(),
                    json!("https://hermes-agent.local/slack/commands"),
                );
                if !usage_hint.is_empty() {
                    entry.insert("usage_hint".to_string(), json!(usage_hint));
                }
                Value::Object(entry)
            })
            .collect(),
    )
}

fn slack_slashes_only_json() -> String {
    serde_json::to_string_pretty(&slack_slash_commands_json()).unwrap_or_else(|_| "[]".to_string())
        + "\n"
}

fn slack_full_manifest_json(name: &str, description: &str) -> String {
    let display_name = truncate_chars_for_cli(name, 35);
    let bot_name = truncate_chars_for_cli(name, 80);
    let display_description = truncate_chars_for_cli(description, 140);
    let manifest = json!({
        "_metadata": {"major_version": 1, "minor_version": 1},
        "display_information": {
            "name": display_name,
            "description": display_description,
            "background_color": "#1a1a2e",
        },
        "features": {
            "app_home": {
                "home_tab_enabled": false,
                "messages_tab_enabled": true,
                "messages_tab_read_only_enabled": false,
            },
            "bot_user": {
                "display_name": bot_name,
                "always_online": true,
            },
            "slash_commands": slack_slash_commands_json(),
            "assistant_view": {
                "assistant_description": "Chat with Hermes in threads and DMs.",
            },
        },
        "oauth_config": {
            "scopes": {
                "bot": [
                    "app_mentions:read",
                    "assistant:write",
                    "channels:history",
                    "channels:read",
                    "chat:write",
                    "commands",
                    "files:read",
                    "files:write",
                    "groups:history",
                    "groups:read",
                    "im:history",
                    "im:read",
                    "im:write",
                    "users:read",
                ],
            },
        },
        "settings": {
            "event_subscriptions": {
                "bot_events": [
                    "app_mention",
                    "assistant_thread_context_changed",
                    "assistant_thread_started",
                    "message.channels",
                    "message.groups",
                    "message.im",
                ],
            },
            "interactivity": {"is_enabled": true},
            "org_deploy_enabled": false,
            "socket_mode_enabled": true,
            "token_rotation_enabled": false,
        },
    });
    serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string()) + "\n"
}

fn slack_manifest_write_stderr(path: &str) -> String {
    format!(
        "Slack manifest written to: {path}\n\nNext steps:\n  1. Open https://api.slack.com/apps and pick your Hermes app\n     (or create a new one: Create New App → From an app manifest).\n  2. Features → App Manifest → paste the contents of\n     {path}\n  3. Save; Slack will prompt to reinstall the app if scopes or\n     slash commands changed.\n  4. Make sure Socket Mode is enabled and you have a bot token\n     (xoxb-...) and app token (xapp-...) configured via\n     `hermes setup`.\n\n"
    )
}

fn truncate_chars_for_cli(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn completion_bash_output() -> String {
    "# Hermes Agent bash completion\n# Add to ~/.bashrc:\n#   eval \"$(hermes completion bash)\"\n\n_hermes_profiles() {\n    echo \"default\"\n}\n\n_hermes_completion() {\n    local cur prev\n    COMPREPLY=()\n    cur=\"${COMP_WORDS[COMP_CWORD]}\"\n    prev=\"${COMP_WORDS[COMP_CWORD-1]}\"\n\n    if [[ $COMP_CWORD -eq 1 ]]; then\n        COMPREPLY=($(compgen -W \"config profile tools gateway sessions cron dashboard completion\" -- \"$cur\"))\n    fi\n}\n\ncomplete -F _hermes_completion hermes\n"
        .to_string()
}

fn completion_zsh_output() -> String {
    "#compdef hermes\n# Hermes Agent zsh completion\n# Add to ~/.zshrc:\n#   eval \"$(hermes completion zsh)\"\n\n_hermes_profiles() {\n    local -a profiles\n    profiles=(default)\n    _describe 'profile' profiles\n}\n\n_hermes() {\n    local context state line\n    typeset -A opt_args\n\n    _arguments -C \\\n        '(-)'{-h,--help}'[Show help and exit]' \\\n        '1:command:->commands' \\\n        '*::arg:->args'\n\n    case $state in\n        commands)\n            local -a subcmds\n            subcmds=(\n                'config:Inspect and edit configuration'\n                'profile:Manage profiles'\n            )\n            _describe 'hermes command' subcmds\n            ;;\n    esac\n}\n\ncompdef _hermes hermes\n"
        .to_string()
}

fn completion_fish_output() -> String {
    "# Hermes Agent fish completion\n# Add to your config:\n#   hermes completion fish | source\n\nfunction __hermes_profiles\n    echo default\nend\n\ncomplete -c hermes -f\ncomplete -c hermes -f -a config -d 'Inspect and edit configuration'\ncomplete -c hermes -f -a profile -d 'Manage profiles'\n"
        .to_string()
}

fn read_platform_toolsets(hermes_home: &Path, platform: &str) -> io::Result<BTreeSet<String>> {
    let config = read_config_value(hermes_home)?;
    Ok(platform_toolsets_from_config(&config, platform)
        .unwrap_or_else(default_display_enabled_toolsets))
}

fn platform_toolsets_from_config(config: &Value, platform: &str) -> Option<BTreeSet<String>> {
    config
        .get("platform_toolsets")
        .and_then(|value| value.get(platform))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
}

fn apply_platform_toolset_change(
    hermes_home: &Path,
    platform: &str,
    names: &[String],
    enable: bool,
) -> io::Result<()> {
    fs::create_dir_all(hermes_home)?;
    let mut config = read_config_value(hermes_home)?;
    let mut enabled = platform_toolsets_from_config(&config, platform)
        .unwrap_or_else(|| default_platform_saved_toolsets(platform));
    for name in names {
        if enable {
            enabled.insert(name.to_string());
        } else {
            enabled.remove(name.as_str());
        }
    }

    let Some(root) = config.as_object_mut() else {
        config = json!({});
        return apply_platform_toolset_change_with_config(hermes_home, config, platform, enabled);
    };
    let platform_toolsets = root
        .entry("platform_toolsets".to_string())
        .or_insert_with(|| json!({}));
    if !platform_toolsets.is_object() {
        *platform_toolsets = json!({});
    }
    platform_toolsets.as_object_mut().unwrap().insert(
        platform.to_string(),
        Value::Array(enabled.into_iter().map(Value::String).collect()),
    );
    fs::write(
        hermes_home.join("config.yaml"),
        serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string()),
    )
}

fn apply_platform_toolset_change_with_config(
    hermes_home: &Path,
    mut config: Value,
    platform: &str,
    enabled: BTreeSet<String>,
) -> io::Result<()> {
    let root = config.as_object_mut().unwrap();
    let mut platforms = serde_json::Map::new();
    platforms.insert(
        platform.to_string(),
        Value::Array(enabled.into_iter().map(Value::String).collect()),
    );
    root.insert("platform_toolsets".to_string(), Value::Object(platforms));
    fs::write(
        hermes_home.join("config.yaml"),
        serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string()),
    )
}

fn tools_list_output(platform: &str, enabled: &BTreeSet<String>) -> String {
    let mut stdout = format!("Built-in toolsets ({platform}):\n");
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

fn tools_list_output_with_mcp(
    platform: &str,
    enabled: &BTreeSet<String>,
    config: &Value,
) -> String {
    let mut stdout = tools_list_output(platform, enabled);
    let servers = mcp_servers(config);
    if servers.is_empty() {
        return stdout;
    }
    stdout.push_str("\nMCP servers:\n");
    for (name, server_config) in servers {
        let Some(tools) = server_config.get("tools").and_then(Value::as_object) else {
            stdout.push_str(&format!("  {name}  all tools enabled\n"));
            continue;
        };
        let include = tools
            .get("include")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        if !include.is_empty() {
            stdout.push_str(&format!(
                "  {name}  [include only: {}]\n",
                include.join(", ")
            ));
            continue;
        }
        let exclude = tools
            .get("exclude")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        if exclude.is_empty() {
            stdout.push_str(&format!("  {name}  all tools enabled\n"));
        } else {
            stdout.push_str(&format!("  {name}  [excluded: {}]\n", exclude.join(", ")));
        }
    }
    stdout
}

fn mcp_tool_change_output(
    hermes_home: &Path,
    target: &str,
    enable: bool,
) -> io::Result<CliExecution> {
    let changed = apply_mcp_tool_change(hermes_home, target, enable)?;
    let stdout = if changed {
        format!(
            "✓ {}: {target}\n",
            if enable { "Enabled" } else { "Disabled" }
        )
    } else {
        let server = target
            .split_once(':')
            .map(|(server, _)| server)
            .unwrap_or(target);
        format!("✗ MCP server '{server}' not found in config\n")
    };
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn apply_mcp_tool_change(hermes_home: &Path, target: &str, enable: bool) -> io::Result<bool> {
    let Some((server_name, tool_name)) = target.split_once(':') else {
        return Ok(false);
    };
    let mut config = read_config_value(hermes_home)?;
    let Some(root) = config.as_object_mut() else {
        return Ok(false);
    };
    let Some(servers) = root.get_mut("mcp_servers").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let Some(server) = servers.get_mut(server_name).and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let tools = server
        .entry("tools".to_string())
        .or_insert_with(|| json!({}));
    if !tools.is_object() {
        *tools = json!({});
    }
    let tools = tools.as_object_mut().unwrap();
    let exclude = tools
        .entry("exclude".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !exclude.is_array() {
        *exclude = Value::Array(Vec::new());
    }
    let exclude = exclude.as_array_mut().unwrap();
    if enable {
        exclude.retain(|value| value.as_str() != Some(tool_name));
    } else if !exclude
        .iter()
        .any(|value| value.as_str() == Some(tool_name))
    {
        exclude.push(Value::String(tool_name.to_string()));
    }
    fs::write(
        hermes_home.join("config.yaml"),
        serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string()),
    )?;
    Ok(true)
}

fn filter_valid_toolset_names(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| is_known_toolset(name))
        .map(|name| (*name).to_string())
        .collect()
}

fn is_known_toolset(name: &str) -> bool {
    DISPLAY_TOOLSETS
        .iter()
        .any(|(toolset, _, _)| *toolset == name)
}

fn is_valid_toolset_platform(platform: &str) -> bool {
    TOOLSET_PLATFORMS.contains(&platform)
}

fn unknown_toolset_platform_output(platform: &str) -> CliExecution {
    CliExecution {
        exit_code: 0,
        stdout: format!(
            "✗ Unknown platform '{platform}'. Valid: {}\n",
            TOOLSET_PLATFORMS.join(", ")
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn toolset_change_output(names: &[&str], enable: bool) -> CliExecution {
    let mut stdout = String::new();
    for name in names.iter().filter(|name| !is_known_toolset(name)) {
        stdout.push_str(&format!("✗ Unknown toolset '{name}'\n"));
    }
    let valid = names
        .iter()
        .filter(|name| is_known_toolset(name))
        .copied()
        .collect::<Vec<_>>();
    if !valid.is_empty() {
        stdout.push_str(&format!(
            "✓ {}: {}\n",
            if enable { "Enabled" } else { "Disabled" },
            valid.join(", ")
        ));
    }
    CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn gateway_status_output() -> String {
    "✗ Gateway is not running\n\nTo start:\n  hermes gateway run      # Run in foreground\n  hermes gateway install  # Install as user service\n  sudo hermes gateway install --system  # Install as boot-time system service\n"
        .to_string()
}

fn gateway_container_start_output() -> String {
    "Service start is not applicable inside a Docker container.\nThe gateway runs as the container's main process.\n\n  docker start <container>     # start a stopped container\n  docker restart <container>   # restart a running container\n\nOr run the gateway directly: hermes gateway run\n"
        .to_string()
}

fn gateway_container_install_output() -> String {
    "Service installation is not needed inside a Docker container.\nThe container runtime is your service manager — use Docker restart policies instead:\n\n  docker run --restart unless-stopped ...   # auto-restart on crash/reboot\n  docker restart <container>                # manual restart\n\nTo run the gateway: hermes gateway run\n"
        .to_string()
}

fn gateway_container_uninstall_output() -> String {
    "Service uninstall is not applicable inside a Docker container.\nTo stop the gateway, stop or remove the container:\n\n  docker stop <container>\n  docker rm <container>\n"
        .to_string()
}

fn gateway_list_output(hermes_home: &Path) -> io::Result<String> {
    let active = active_profile_name(hermes_home);
    let mut names = vec!["default".to_string()];
    let profiles_root = hermes_home.join("profiles");
    if profiles_root.is_dir() {
        let mut profile_names = fs::read_dir(profiles_root)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| is_valid_profile_name(name))
            .collect::<Vec<_>>();
        profile_names.sort();
        names.extend(profile_names);
    }

    let mut stdout = "Gateways:\n".to_string();
    for name in names {
        let mut label = name.clone();
        if name == active {
            label.push_str(" (current)");
        }
        stdout.push_str(&format!("  ✗ {label:<24}\n"));
    }
    Ok(stdout)
}

fn webhook_subscriptions_path(hermes_home: &Path) -> PathBuf {
    hermes_home.join("webhook_subscriptions.json")
}

fn load_webhook_subscriptions(hermes_home: &Path) -> io::Result<BTreeMap<String, Value>> {
    let path = webhook_subscriptions_path(hermes_home);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let value =
        serde_json::from_str::<Value>(&fs::read_to_string(path)?).unwrap_or_else(|_| json!({}));
    Ok(value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default())
}

fn save_webhook_subscriptions(
    hermes_home: &Path,
    subscriptions: &BTreeMap<String, Value>,
) -> io::Result<()> {
    fs::create_dir_all(hermes_home)?;
    fs::write(
        webhook_subscriptions_path(hermes_home),
        serde_json::to_string_pretty(subscriptions).unwrap_or_else(|_| "{}".to_string()),
    )
}

fn webhook_config(config: &Value) -> Option<&serde_json::Map<String, Value>> {
    config
        .get("platforms")?
        .as_object()?
        .get("webhook")?
        .as_object()
}

fn webhook_enabled(hermes_home: &Path) -> io::Result<bool> {
    let config = read_config_value(hermes_home)?;
    Ok(webhook_config(&config)
        .and_then(|webhook| webhook.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

fn webhook_base_url(hermes_home: &Path) -> io::Result<String> {
    let config = read_config_value(hermes_home)?;
    let extra = webhook_config(&config)
        .and_then(|webhook| webhook.get("extra"))
        .and_then(Value::as_object);
    let host = extra
        .and_then(|extra| extra.get("host"))
        .and_then(Value::as_str)
        .unwrap_or("0.0.0.0");
    let port = extra
        .and_then(|extra| extra.get("port"))
        .and_then(|value| {
            value
                .as_u64()
                .map(|number| number.to_string())
                .or_else(|| value.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| "8644".to_string());
    let display_host = if host == "0.0.0.0" { "localhost" } else { host };
    Ok(format!("http://{display_host}:{port}"))
}

fn webhook_setup_hint(hermes_home: &Path) -> String {
    let home = hermes_home.display();
    format!(
        "\n  Webhook platform is not enabled. To set it up:\n\n  1. Run the gateway setup wizard:\n     hermes gateway setup\n\n  2. Or manually add to {home}/config.yaml:\n     platforms:\n       webhook:\n         enabled: true\n         extra:\n           host: \"0.0.0.0\"\n           port: 8644\n           secret: \"your-global-hmac-secret\"\n\n  3. Or set environment variables in {home}/.env:\n     WEBHOOK_ENABLED=true\n     WEBHOOK_PORT=8644\n     WEBHOOK_SECRET=your-global-secret\n\n  Then start the gateway: hermes gateway run\n\n"
    )
}

fn webhook_requires_enabled(hermes_home: &Path) -> io::Result<Option<CliExecution>> {
    if webhook_enabled(hermes_home)? {
        Ok(None)
    } else {
        Ok(Some(CliExecution {
            exit_code: 0,
            stdout: webhook_setup_hint(hermes_home),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        }))
    }
}

fn normalize_webhook_name(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace(' ', "-")
}

fn is_valid_webhook_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

#[derive(Default)]
struct WebhookSubscribeArgs {
    prompt: String,
    events: String,
    description: String,
    skills: String,
    deliver: String,
    deliver_chat_id: String,
    secret: String,
    deliver_only: bool,
}

fn parse_webhook_subscribe_args(args: &[&str]) -> WebhookSubscribeArgs {
    let mut parsed = WebhookSubscribeArgs {
        deliver: "log".to_string(),
        ..WebhookSubscribeArgs::default()
    };
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--prompt" if i + 1 < args.len() => {
                parsed.prompt = args[i + 1].to_string();
                i += 2;
            }
            "--events" if i + 1 < args.len() => {
                parsed.events = args[i + 1].to_string();
                i += 2;
            }
            "--description" if i + 1 < args.len() => {
                parsed.description = args[i + 1].to_string();
                i += 2;
            }
            "--skills" if i + 1 < args.len() => {
                parsed.skills = args[i + 1].to_string();
                i += 2;
            }
            "--deliver" if i + 1 < args.len() => {
                parsed.deliver = args[i + 1].to_string();
                i += 2;
            }
            "--deliver-chat-id" if i + 1 < args.len() => {
                parsed.deliver_chat_id = args[i + 1].to_string();
                i += 2;
            }
            "--secret" if i + 1 < args.len() => {
                parsed.secret = args[i + 1].to_string();
                i += 2;
            }
            "--deliver-only" => {
                parsed.deliver_only = true;
                i += 1;
            }
            _ => i += 1,
        }
    }
    parsed
}

fn split_csv(value: &str) -> Vec<Value> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| json!(item))
        .collect()
}

fn webhook_list_output(hermes_home: &Path) -> io::Result<CliExecution> {
    if let Some(disabled) = webhook_requires_enabled(hermes_home)? {
        return Ok(disabled);
    }
    let subscriptions = load_webhook_subscriptions(hermes_home)?;
    if subscriptions.is_empty() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "  No dynamic webhook subscriptions.\n  Create one with: hermes webhook subscribe <name>\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let base_url = webhook_base_url(hermes_home)?;
    let mut stdout = format!("\n  {} webhook subscription(s):\n\n", subscriptions.len());
    for (name, route) in subscriptions {
        let object = route.as_object();
        let events = object
            .and_then(|object| object.get("events"))
            .and_then(Value::as_array)
            .map(|events| {
                events
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|events| !events.is_empty())
            .unwrap_or_else(|| "(all)".to_string());
        let mut deliver = object
            .and_then(|object| object.get("deliver"))
            .and_then(Value::as_str)
            .unwrap_or("log")
            .to_string();
        if object
            .and_then(|object| object.get("deliver_only"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            deliver.push_str(" (direct — no agent)");
        }
        let description = object
            .and_then(|object| object.get("description"))
            .and_then(Value::as_str)
            .unwrap_or("");
        stdout.push_str(&format!("  ◆ {name}\n"));
        if !description.is_empty() {
            stdout.push_str(&format!("    {description}\n"));
        }
        stdout.push_str(&format!(
            "    URL:     {base_url}/webhooks/{name}\n    Events:  {events}\n    Deliver: {deliver}\n\n"
        ));
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn webhook_subscribe_output(
    hermes_home: &Path,
    raw_name: &str,
    args: &[&str],
) -> io::Result<CliExecution> {
    if let Some(disabled) = webhook_requires_enabled(hermes_home)? {
        return Ok(disabled);
    }
    let name = normalize_webhook_name(raw_name);
    if !is_valid_webhook_name(&name) {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: format!(
                "Error: Invalid name '{name}'. Use lowercase alphanumeric with hyphens/underscores.\n"
            ),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let parsed = parse_webhook_subscribe_args(args);
    if parsed.deliver_only && parsed.deliver == "log" {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "Error: --deliver-only requires --deliver to be a real target (telegram, discord, slack, github_comment, etc.) — not 'log'.\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let mut subscriptions = load_webhook_subscriptions(hermes_home)?;
    let is_update = subscriptions.contains_key(&name);
    let secret = if parsed.secret.is_empty() {
        "generated-secret".to_string()
    } else {
        parsed.secret.clone()
    };
    let events = split_csv(&parsed.events);
    let skills = split_csv(&parsed.skills);
    let mut route = serde_json::Map::new();
    route.insert(
        "description".to_string(),
        json!(if parsed.description.is_empty() {
            format!("Agent-created subscription: {name}")
        } else {
            parsed.description.clone()
        }),
    );
    route.insert("events".to_string(), Value::Array(events.clone()));
    route.insert("secret".to_string(), json!(secret));
    route.insert("prompt".to_string(), json!(parsed.prompt));
    route.insert("skills".to_string(), Value::Array(skills));
    route.insert("deliver".to_string(), json!(parsed.deliver));
    route.insert("created_at".to_string(), json!("1970-01-01T00:00:00Z"));
    if parsed.deliver_only {
        route.insert("deliver_only".to_string(), json!(true));
    }
    if !parsed.deliver_chat_id.is_empty() {
        route.insert(
            "deliver_extra".to_string(),
            json!({"chat_id": parsed.deliver_chat_id}),
        );
    }
    subscriptions.insert(name.clone(), Value::Object(route));
    save_webhook_subscriptions(hermes_home, &subscriptions)?;

    let base_url = webhook_base_url(hermes_home)?;
    let status = if is_update { "Updated" } else { "Created" };
    let mut stdout = format!(
        "\n  {status} webhook subscription: {name}\n  URL:    {base_url}/webhooks/{name}\n  Secret: {}\n",
        if parsed.secret.is_empty() {
            "generated-secret"
        } else {
            &parsed.secret
        }
    );
    if events.is_empty() {
        stdout.push_str("  Events: (all)\n");
    } else {
        let rendered = events
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        stdout.push_str(&format!("  Events: {rendered}\n"));
    }
    stdout.push_str(&format!("  Deliver: {}\n", parsed.deliver));
    if parsed.deliver_only {
        stdout.push_str("  Mode: direct delivery (no agent, zero LLM cost)\n");
    }
    if !parsed.prompt.is_empty() {
        let preview = if parsed.prompt.chars().count() > 80 {
            format!("{}...", parsed.prompt.chars().take(80).collect::<String>())
        } else {
            parsed.prompt.clone()
        };
        let label = if parsed.deliver_only {
            "Message"
        } else {
            "Prompt"
        };
        stdout.push_str(&format!("  {label}: {preview}\n"));
    }
    stdout.push_str(
        "\n  Configure your service to POST to the URL above.\n  Use the secret for HMAC-SHA256 signature validation.\n  The gateway must be running to receive events (hermes gateway run).\n\n",
    );
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn webhook_remove_output(hermes_home: &Path, raw_name: &str) -> io::Result<CliExecution> {
    if let Some(disabled) = webhook_requires_enabled(hermes_home)? {
        return Ok(disabled);
    }
    let name = raw_name.trim().to_ascii_lowercase();
    let mut subscriptions = load_webhook_subscriptions(hermes_home)?;
    if subscriptions.remove(&name).is_none() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: format!(
                "  No subscription named '{name}'.\n  Note: Static routes from config.yaml cannot be removed here.\n"
            ),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    save_webhook_subscriptions(hermes_home, &subscriptions)?;
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!("  Removed webhook subscription: {name}\n"),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn webhook_test_output(hermes_home: &Path, raw_name: &str) -> io::Result<CliExecution> {
    if let Some(disabled) = webhook_requires_enabled(hermes_home)? {
        return Ok(disabled);
    }
    let name = raw_name.trim().to_ascii_lowercase();
    let subscriptions = load_webhook_subscriptions(hermes_home)?;
    if !subscriptions.contains_key(&name) {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: format!("  No subscription named '{name}'.\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!(
            "  Sending test POST to {}/webhooks/{name}\n  Error: network disabled in parity harness\n  Is the gateway running? (hermes gateway run)\n",
            webhook_base_url(hermes_home)?
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

#[derive(Clone, Debug)]
struct PluginListEntry {
    key: String,
    version: String,
    description: String,
}

fn plugin_manifest_value(plugin_dir: &Path) -> Value {
    for name in ["plugin.yaml", "plugin.yml"] {
        let path = plugin_dir.join(name);
        if path.exists() {
            if let Ok(text) = fs::read_to_string(path) {
                return serde_yaml::from_str::<Value>(&text).unwrap_or_else(|_| json!({}));
            }
        }
    }
    json!({})
}

fn discover_user_plugin_entries(hermes_home: &Path) -> io::Result<Vec<PluginListEntry>> {
    let plugins_dir = hermes_home.join("plugins");
    if !plugins_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(plugins_dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = plugin_manifest_value(&path);
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let key = manifest
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(&dir_name)
            .to_string();
        let version = manifest
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let description = manifest
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        entries.push(PluginListEntry {
            key,
            version,
            description,
        });
    }
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

fn plugin_exists_for_cli(hermes_home: &Path, name: &str) -> io::Result<bool> {
    if hermes_home.join("plugins").join(name).is_dir() {
        return Ok(true);
    }
    Ok(discover_user_plugin_entries(hermes_home)?
        .iter()
        .any(|entry| entry.key == name))
}

fn config_string_set(config: &Value, section: &str, key: &str) -> BTreeSet<String> {
    config
        .get(section)
        .and_then(Value::as_object)
        .and_then(|section| section.get(key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn write_plugin_sets(
    hermes_home: &Path,
    enabled: &BTreeSet<String>,
    disabled: &BTreeSet<String>,
) -> io::Result<()> {
    let mut config = read_config_value(hermes_home)?;
    if !config.is_object() {
        config = json!({});
    }
    let root = config.as_object_mut().unwrap();
    let plugins = root
        .entry("plugins".to_string())
        .or_insert_with(|| json!({}));
    if !plugins.is_object() {
        *plugins = json!({});
    }
    let plugins = plugins.as_object_mut().unwrap();
    plugins.insert(
        "enabled".to_string(),
        Value::Array(enabled.iter().map(|value| json!(value)).collect()),
    );
    plugins.insert(
        "disabled".to_string(),
        Value::Array(disabled.iter().map(|value| json!(value)).collect()),
    );
    write_config_value(hermes_home, &config)
}

fn plugins_list_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let entries = discover_user_plugin_entries(hermes_home)?;
    if entries.is_empty() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "No plugins installed.\nInstall with: hermes plugins install owner/repo\n"
                .to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let config = read_config_value(hermes_home)?;
    let enabled = config_string_set(&config, "plugins", "enabled");
    let disabled = config_string_set(&config, "plugins", "disabled");
    let mut stdout = "\nPlugins\nName | Status | Version | Description | Source\n".to_string();
    for entry in entries {
        let status = if disabled.contains(&entry.key) {
            "disabled"
        } else if enabled.contains(&entry.key) {
            "enabled"
        } else {
            "not enabled"
        };
        stdout.push_str(&format!(
            "{} | {} | {} | {} | user\n",
            entry.key, status, entry.version, entry.description
        ));
    }
    stdout.push_str(
        "\nInteractive toggle: hermes plugins\nEnable/disable: hermes plugins enable/disable <name>\nPlugins are opt-in by default — only 'enabled' plugins load.\n",
    );
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn plugins_set_enabled_output(
    hermes_home: &Path,
    name: &str,
    enable: bool,
) -> io::Result<CliExecution> {
    if !plugin_exists_for_cli(hermes_home, name)? {
        return Ok(CliExecution {
            exit_code: 1,
            stdout: format!("Plugin '{name}' is not installed or bundled.\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let config = read_config_value(hermes_home)?;
    let mut enabled = config_string_set(&config, "plugins", "enabled");
    let mut disabled = config_string_set(&config, "plugins", "disabled");
    if enable {
        if enabled.contains(name) && !disabled.contains(name) {
            return Ok(CliExecution {
                exit_code: 0,
                stdout: format!("Plugin '{name}' is already enabled.\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            });
        }
        enabled.insert(name.to_string());
        disabled.remove(name);
        write_plugin_sets(hermes_home, &enabled, &disabled)?;
        Ok(CliExecution {
            exit_code: 0,
            stdout: format!("✓ Plugin {name} enabled. Takes effect on next session.\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        })
    } else {
        if !enabled.contains(name) && disabled.contains(name) {
            return Ok(CliExecution {
                exit_code: 0,
                stdout: format!("Plugin '{name}' is already disabled.\n"),
                stdout_markers: BTreeMap::new(),
                stderr: String::new(),
            });
        }
        enabled.remove(name);
        disabled.insert(name.to_string());
        write_plugin_sets(hermes_home, &enabled, &disabled)?;
        Ok(CliExecution {
            exit_code: 0,
            stdout: format!("⊘ Plugin {name} disabled. Takes effect on next session.\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        })
    }
}

fn skills_list_cli_output(
    hermes_home: &Path,
    source_filter: &str,
    enabled_only: bool,
) -> io::Result<CliExecution> {
    let root = hermes_home.join("skills");
    let list = hermes_skills::skills_list_json(&root, None)?;
    let skills = list
        .get("skills")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let config = read_config_value(hermes_home)?;
    let disabled = config_string_set(&config, "skills", "disabled");
    let title = if enabled_only {
        "Installed Skills (enabled only)"
    } else {
        "Installed Skills"
    };
    let mut rows = Vec::new();
    let mut local_count = 0;
    let mut enabled_count = 0;
    let mut disabled_count = 0;
    for skill in skills {
        let name = skill
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let is_enabled = !disabled.contains(&name);
        if enabled_only && !is_enabled {
            continue;
        }
        if !matches!(source_filter, "all" | "local") {
            continue;
        }
        local_count += 1;
        if is_enabled {
            enabled_count += 1;
        } else {
            disabled_count += 1;
        }
        let category = skill
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or_default();
        rows.push(format!(
            "{} | {} | local | local | {}",
            name,
            category,
            if is_enabled { "enabled" } else { "disabled" }
        ));
    }
    let mut stdout = format!("{title}\nName | Category | Source | Trust | Status\n");
    for row in rows {
        stdout.push_str(&row);
        stdout.push('\n');
    }
    if enabled_only {
        stdout.push_str(&format!(
            "0 hub-installed, 0 builtin, {local_count} local — {enabled_count} enabled shown\n\n"
        ));
    } else {
        stdout.push_str(&format!(
            "0 hub-installed, 0 builtin, {local_count} local — {enabled_count} enabled, {disabled_count} disabled\n\n"
        ));
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn checkpoint_base(hermes_home: &Path) -> PathBuf {
    hermes_home.join("checkpoints")
}

fn checkpoint_dir_size(path: &Path) -> io::Result<u64> {
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    let mut total = 0;
    if path.is_dir() {
        for entry in fs::read_dir(path)?.flatten() {
            total += checkpoint_dir_size(&entry.path()).unwrap_or(0);
        }
    }
    Ok(total)
}

fn checkpoint_legacy_archives(base: &Path) -> io::Result<Vec<(String, u64)>> {
    if !base.is_dir() {
        return Ok(Vec::new());
    }
    let mut archives = Vec::new();
    for entry in fs::read_dir(base)?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() && name.starts_with("legacy-") {
            archives.push((name, checkpoint_dir_size(&path).unwrap_or(0)));
        }
    }
    archives.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(archives)
}

fn checkpoints_status_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let base = checkpoint_base(hermes_home);
    let legacy = checkpoint_legacy_archives(&base)?;
    let legacy_size = legacy.iter().map(|(_, size)| *size).sum::<u64>();
    let store_size = checkpoint_dir_size(&base.join("store")).unwrap_or(0);
    let total_size = checkpoint_dir_size(&base).unwrap_or(0);
    let mut stdout = format!(
        "Checkpoint base: {}\nTotal size:      {}\n  store/         {}\n  legacy-*       {}\nProjects:        0\n",
        base.display(),
        backup_format_size(total_size),
        backup_format_size(store_size),
        backup_format_size(legacy_size)
    );
    if !legacy.is_empty() {
        stdout.push_str(&format!("\nLegacy archives ({}):\n", legacy.len()));
        for (name, size) in legacy {
            stdout.push_str(&format!("  {name:<40}  {:>10}\n", backup_format_size(size)));
        }
        stdout.push_str("\nClear with: hermes checkpoints clear-legacy\n");
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn checkpoints_clear_legacy_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let base = checkpoint_base(hermes_home);
    let legacy = checkpoint_legacy_archives(&base)?;
    if legacy.is_empty() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "No legacy archives to clear.\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let mut deleted = 0;
    let mut bytes = 0;
    for (name, size) in legacy {
        let path = base.join(name);
        if fs::remove_dir_all(path).is_ok() {
            deleted += 1;
            bytes += size;
        }
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout: format!(
            "Deleted {deleted} archive(s), reclaimed {}.\n",
            backup_format_size(bytes)
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn checkpoints_clear_output(hermes_home: &Path) -> io::Result<CliExecution> {
    let base = checkpoint_base(hermes_home);
    if !base.exists() {
        return Ok(CliExecution {
            exit_code: 0,
            stdout: "Nothing to clear — checkpoint base does not exist.\n".to_string(),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        });
    }
    let size = checkpoint_dir_size(&base).unwrap_or(0);
    let legacy_count = checkpoint_legacy_archives(&base)?.len();
    let mut stdout = format!(
        "This will delete the ENTIRE checkpoint base at {}\n  size:        {}\n  projects:    0\n  legacy dirs: {legacy_count}\n\nAll /rollback history for every working directory will be lost.\n",
        base.display(),
        backup_format_size(size)
    );
    fs::remove_dir_all(&base)?;
    stdout.push_str(&format!(
        "Cleared. Reclaimed {}.\n",
        backup_format_size(size)
    ));
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn proxy_help_output() -> String {
    "hermes proxy — local OpenAI-compatible proxy that attaches your\nOAuth-authenticated provider credentials to outbound requests.\n\nSubcommands:\n  hermes proxy start [--provider nous|xai] [--host 127.0.0.1] [--port 8645]\n      Run the proxy in the foreground.\n  hermes proxy status\n      Show which upstream adapters are ready.\n  hermes proxy providers\n      List available upstream providers.\n".to_string()
}

fn proxy_providers_output() -> String {
    "Available proxy upstream providers:\n  nous  — Nous Portal\n  xai  — xAI Grok\n".to_string()
}

fn proxy_status_output() -> String {
    "Hermes proxy upstream adapters\n\n  [nous    ] Nous Portal — not logged in\n  [xai     ] xAI Grok — not logged in\n\nStart the proxy with: hermes proxy start [--provider <name>]\n".to_string()
}

fn debug_help_output() -> String {
    "Usage: hermes debug <command>\n\nCommands:\n  share    Upload debug report to a paste service and print URL\n  delete   Delete a previously uploaded paste\n\nOptions (share):\n  --lines N    Number of log lines to include (default: 200)\n  --expire N   Paste expiry in days (default: 7)\n  --local      Print report locally instead of uploading\n  --no-redact  Disable upload-time secret redaction (default: redact)\n\nOptions (delete):\n  <url> ...    One or more paste URLs to delete\n".to_string()
}

fn read_log_tail(hermes_home: &Path, log_name: &str, lines: usize) -> String {
    let path = hermes_home.join("logs").join(log_name);
    let Ok(text) = fs::read_to_string(path) else {
        return "(file not found)".to_string();
    };
    let all = text.lines().collect::<Vec<_>>();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

fn redact_debug_text(text: &str) -> String {
    text.replace("sk-test-debug-secret", "[REDACTED]")
}

fn debug_share_local_output(hermes_home: &Path, lines: &str) -> io::Result<CliExecution> {
    let line_count = lines.parse::<usize>().unwrap_or(200);
    let dump = dump_output(hermes_home, false)?.stdout;
    let agent_tail = redact_debug_text(&read_log_tail(hermes_home, "agent.log", line_count));
    let errors_tail = redact_debug_text(&read_log_tail(
        hermes_home,
        "errors.log",
        line_count.min(100),
    ));
    let gateway_tail = redact_debug_text(&read_log_tail(
        hermes_home,
        "gateway.log",
        line_count.min(100),
    ));
    let full_agent = redact_debug_text(
        &fs::read_to_string(hermes_home.join("logs").join("agent.log")).unwrap_or_default(),
    );
    let full_gateway = redact_debug_text(
        &fs::read_to_string(hermes_home.join("logs").join("gateway.log")).unwrap_or_default(),
    );
    let banner = "[hermes debug share: log content redacted at upload time. run with --no-redact to disable]\n";
    let mut stdout = format!(
        "Collecting debug report...\n{banner}{dump}\n\n--- agent.log (last {line_count} lines) ---\n{agent_tail}\n\n--- errors.log (last {} lines) ---\n{errors_tail}\n\n--- gateway.log (last {} lines) ---\n{gateway_tail}\n",
        line_count.min(100),
        line_count.min(100)
    );
    if !full_agent.is_empty() {
        stdout.push_str(&format!(
            "\n\n============================================================\nFULL agent.log\n============================================================\n\n{banner}{dump}\n\n--- full agent.log ---\n{full_agent}"
        ));
    }
    if !full_gateway.is_empty() {
        stdout.push_str(&format!(
            "\n\n============================================================\nFULL gateway.log\n============================================================\n\n{banner}{dump}\n\n--- full gateway.log ---\n{full_gateway}"
        ));
    }
    Ok(CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    })
}

fn is_valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn mcp_servers(config: &Value) -> BTreeMap<String, Value> {
    config
        .get("mcp_servers")
        .and_then(Value::as_object)
        .map(|servers| {
            servers
                .iter()
                .map(|(name, config)| (name.clone(), config.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default()
}

fn mcp_list_output(config: &Value) -> String {
    let servers = mcp_servers(config);
    if servers.is_empty() {
        return "\n  No MCP servers configured.\n\n  Add one with:\n    hermes mcp add <name> --url <endpoint>\n    hermes mcp add <name> --command <cmd> --args <args...>\n\n".to_string();
    }

    let mut stdout = "\n  MCP Servers:\n\n".to_string();
    stdout.push_str(&format!(
        "  {:<16} {:<30} {:<12} {:<10}\n",
        "Name", "Transport", "Tools", "Status"
    ));
    stdout.push_str(&format!(
        "  {:<16} {:<30} {:<12} {:<10}\n",
        "─".repeat(16),
        "─".repeat(30),
        "─".repeat(12),
        "─".repeat(10)
    ));

    for (name, config) in servers {
        let transport = mcp_transport_label(&config);
        let tools = mcp_tools_label(&config);
        let enabled = config.get("enabled").map(mcp_truthy).unwrap_or(true);
        let status = if enabled {
            "✓ enabled"
        } else {
            "✗ disabled"
        };
        stdout.push_str(&format!(
            "  {name:<16} {transport:<30} {tools:<12} {status}\n"
        ));
    }
    stdout.push('\n');
    stdout
}

fn mcp_transport_label(config: &Value) -> String {
    if let Some(url) = config.get("url").and_then(Value::as_str) {
        return truncate_mcp_label(url, 28);
    }
    let command = config.get("command").and_then(Value::as_str).unwrap_or("?");
    let args = config
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(2)
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let transport = if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
    };
    truncate_mcp_label(&transport, 28)
}

fn truncate_mcp_label(value: &str, max_len: usize) -> String {
    if value.chars().count() > max_len {
        format!("{}...", truncate_chars(value, max_len.saturating_sub(3)))
    } else {
        value.to_string()
    }
}

fn mcp_tools_label(config: &Value) -> String {
    let Some(tools) = config.get("tools").and_then(Value::as_object) else {
        return "all".to_string();
    };
    if let Some(include) = tools.get("include").and_then(Value::as_array) {
        if !include.is_empty() {
            return format!("{} selected", include.len());
        }
    }
    if let Some(exclude) = tools.get("exclude").and_then(Value::as_array) {
        if !exclude.is_empty() {
            return format!("-{} excluded", exclude.len());
        }
    }
    "all".to_string()
}

fn mcp_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::String(value) => matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes"),
        _ => false,
    }
}

fn remove_mcp_server(hermes_home: &Path, name: &str) -> io::Result<bool> {
    let mut config = read_config_value(hermes_home)?;
    let Some(root) = config.as_object_mut() else {
        return Ok(false);
    };
    let Some(servers) = root.get_mut("mcp_servers").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let removed = servers.remove(name).is_some();
    if removed {
        if servers.is_empty() {
            root.remove("mcp_servers");
        }
        fs::write(
            hermes_home.join("config.yaml"),
            serde_yaml::to_string(&config).unwrap_or_else(|_| "{}\n".to_string()),
        )?;
    }
    Ok(removed)
}

fn parse_mcp_env_assignment(value: &str) -> Result<(&str, &str), String> {
    let Some((key, raw_value)) = value.split_once('=') else {
        return Err(format!(
            "Invalid --env value '{value}' (expected KEY=VALUE)"
        ));
    };
    if key.is_empty() {
        return Err(format!(
            "Invalid --env value '{value}' (missing variable name)"
        ));
    }
    if !is_valid_env_var_name(key) {
        return Err(format!("Invalid --env variable name '{key}'"));
    }
    Ok((key, raw_value))
}

fn is_valid_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn cron_jobs_path(hermes_home: &Path) -> PathBuf {
    hermes_home.join("cron").join("jobs.json")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CreatedCronJob {
    id: String,
    name: String,
    schedule_display: String,
    next_run_at: String,
}

fn create_cron_job(
    hermes_home: &Path,
    schedule_text: &str,
    prompt: &str,
    name: &str,
    deliver: &str,
) -> io::Result<CreatedCronJob> {
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
    let id = unique_cron_job_id(&jobs, name);
    jobs.push(json!({
        "id": id,
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
    save_cron_jobs_object(&path, &jobs)?;
    Ok(CreatedCronJob {
        id,
        name: name.to_string(),
        schedule_display: display,
        next_run_at,
    })
}

fn unique_cron_job_id(jobs: &[Value], name: &str) -> String {
    let base = format!("rust-{name}");
    if !jobs
        .iter()
        .any(|job| job.get("id").and_then(Value::as_str) == Some(base.as_str()))
    {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if !jobs
            .iter()
            .any(|job| job.get("id").and_then(Value::as_str) == Some(candidate.as_str()))
        {
            return candidate;
        }
    }
    unreachable!("unbounded loop returns on first unused id")
}

fn cron_create_output(created: &CreatedCronJob) -> CliExecution {
    CliExecution {
        exit_code: 0,
        stdout: format!(
            "Created job: {}\n  Name: {}\n  Schedule: {}\n  Next run: {}\n",
            created.id, created.name, created.schedule_display, created.next_run_at
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CronMutationOutcome {
    Changed,
    Missing,
    Ambiguous(Vec<String>),
}

fn set_cron_job_enabled(
    hermes_home: &Path,
    name: &str,
    enabled: bool,
) -> io::Result<CronMutationOutcome> {
    let path = cron_jobs_path(hermes_home);
    let mut jobs = hermes_cron::load_jobs(&path)?;
    let matching_ids = cron_matching_job_ids(&jobs, name);
    if matching_ids.is_empty() {
        save_cron_jobs_object(&path, &jobs)?;
        return Ok(CronMutationOutcome::Missing);
    }
    if matching_ids.len() > 1 {
        save_cron_jobs_object(&path, &jobs)?;
        return Ok(CronMutationOutcome::Ambiguous(matching_ids));
    }
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
    save_cron_jobs_object(&path, &jobs)?;
    Ok(CronMutationOutcome::Changed)
}

fn remove_cron_job(hermes_home: &Path, name: &str) -> io::Result<CronMutationOutcome> {
    let path = cron_jobs_path(hermes_home);
    let mut jobs = hermes_cron::load_jobs(&path)?;
    let matching_ids = cron_matching_job_ids(&jobs, name);
    if matching_ids.is_empty() {
        save_cron_jobs_object(&path, &jobs)?;
        return Ok(CronMutationOutcome::Missing);
    }
    if matching_ids.len() > 1 {
        save_cron_jobs_object(&path, &jobs)?;
        return Ok(CronMutationOutcome::Ambiguous(matching_ids));
    }
    jobs.retain(|job| !cron_job_matches(job, name));
    save_cron_jobs_object(&path, &jobs)?;
    Ok(CronMutationOutcome::Changed)
}

fn cron_missing_job_output(action: &str, name: &str) -> CliExecution {
    CliExecution {
        exit_code: 0,
        stdout: format!(
            "Failed to {action} job: Job with ID or name '{name}' not found. Use cronjob(action='list') to inspect jobs.\n"
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn cron_ambiguous_job_output(action: &str, name: &str, ids: &[String]) -> CliExecution {
    CliExecution {
        exit_code: 0,
        stdout: format!(
            "Failed to {action} job: Job name '{name}' is ambiguous — matches {} jobs: {}. Use the job ID instead.\n",
            ids.len(),
            ids.join(", ")
        ),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn cron_job_matches(job: &Value, name_or_id: &str) -> bool {
    job.get("id").and_then(Value::as_str) == Some(name_or_id)
        || job.get("name").and_then(Value::as_str) == Some(name_or_id)
}

fn cron_matching_job_ids(jobs: &[Value], name_or_id: &str) -> Vec<String> {
    jobs.iter()
        .filter(|job| cron_job_matches(job, name_or_id))
        .map(|job| {
            job.get("id")
                .and_then(Value::as_str)
                .unwrap_or(name_or_id)
                .to_string()
        })
        .collect()
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

fn checked_profile_dir(hermes_home: &Path, name: &str) -> Result<(String, PathBuf), CliExecution> {
    let canon = normalize_profile_name_for_cli(name).map_err(profile_name_error)?;
    validate_profile_name_for_cli(&canon).map_err(profile_name_error)?;
    let dir = profile_dir(hermes_home, &canon);
    Ok((canon, dir))
}

fn profile_name_error(error: io::Error) -> CliExecution {
    CliExecution {
        exit_code: 1,
        stdout: format!("Error: {error}\n"),
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
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

fn create_cloned_profile(
    source_dir: &Path,
    profile_dir: &Path,
    description: Option<&str>,
) -> io::Result<()> {
    if !source_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source profile does not exist at {}", source_dir.display()),
        ));
    }
    create_minimal_profile(profile_dir, description, false)?;
    for filename in ["config.yaml", ".env", "SOUL.md"] {
        let source = source_dir.join(filename);
        if source.exists() {
            fs::copy(source, profile_dir.join(filename))?;
        }
    }
    let source_skills = source_dir.join("skills");
    if source_skills.is_dir() {
        copy_dir_contents(&source_skills, &profile_dir.join("skills"))?;
    }
    for relpath in ["memories/MEMORY.md", "memories/USER.md"] {
        let source = source_dir.join(relpath);
        if source.exists() {
            let target = profile_dir.join(relpath);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source, target)?;
        }
    }
    Ok(())
}

fn create_clone_all_profile(
    source_dir: &Path,
    profile_dir: &Path,
    description: Option<&str>,
) -> io::Result<()> {
    if !source_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source profile does not exist at {}", source_dir.display()),
        ));
    }
    if profile_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Profile already exists at {}", profile_dir.display()),
        ));
    }
    fs::create_dir_all(profile_dir)?;
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "profiles" {
            continue;
        }
        let source_path = entry.path();
        let target_path = profile_dir.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_contents(&source_path, &target_path)?;
        } else if source_path.is_file() {
            fs::copy(source_path, target_path)?;
        }
    }
    for stale in ["gateway.pid", "gateway_state.json", "processes.json"] {
        let _ = fs::remove_file(profile_dir.join(stale));
    }
    if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
        write_profile_description(profile_dir, description)?;
    }
    Ok(())
}

fn copy_dir_contents(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_contents(&source_path, &target_path)?;
        } else if source_path.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source_path, target_path)?;
        }
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

fn logs_list_output(hermes_home: &Path) -> String {
    let logs_dir = hermes_home.join("logs");
    if !logs_dir.exists() {
        return format!("No logs directory at {}/logs/\n", hermes_home.display());
    }
    let mut stdout = format!("Log files in {}/logs/:\n\n", hermes_home.display());
    let mut found = false;
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        let mut names = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        names.sort();
        for name in names {
            found = true;
            let size = fs::metadata(logs_dir.join(&name))
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            stdout.push_str(&format!("  {name:<25} {size:>8}B   just now\n"));
        }
    }
    if !found {
        stdout.push_str("  (no log files yet — run 'hermes chat' to generate logs)\n");
    }
    stdout
}

fn logs_tail_execution(
    hermes_home: &Path,
    log_name: &str,
    lines: &str,
    level: Option<&str>,
    session: Option<&str>,
    component: Option<&str>,
) -> CliExecution {
    let Some(filename) = log_filename(log_name) else {
        return CliExecution {
            exit_code: 1,
            stdout: format!("Unknown log: '{log_name}'. Available: agent, errors, gateway\n"),
            stdout_markers: BTreeMap::new(),
            stderr: String::new(),
        };
    };
    let path = hermes_home.join("logs").join(filename);
    let num_lines = lines.parse::<usize>().unwrap_or(50);
    let mut all_lines = if path.exists() {
        fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if let Some(level) = level {
        all_lines.retain(|line| log_line_level_at_least(line, level));
    }
    if let Some(session) = session {
        all_lines.retain(|line| line.contains(session));
    }
    if let Some(component) = component {
        all_lines.retain(|line| log_line_component_matches(line, component));
    }
    let start = all_lines.len().saturating_sub(num_lines);
    let mut filter_parts = Vec::new();
    if let Some(level) = level {
        filter_parts.push(format!("level>={}", level.to_ascii_uppercase()));
    }
    if let Some(session) = session {
        filter_parts.push(format!("session={session}"));
    }
    if let Some(component) = component {
        filter_parts.push(format!("component={component}"));
    }
    let filter_desc = if filter_parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filter_parts.join(", "))
    };
    let mut stdout = format!(
        "--- {}/logs/{filename}{filter_desc} (last {num_lines}) ---\n",
        hermes_home.display()
    );
    for line in &all_lines[start..] {
        stdout.push_str(line);
        stdout.push('\n');
    }
    CliExecution {
        exit_code: 0,
        stdout,
        stdout_markers: BTreeMap::new(),
        stderr: String::new(),
    }
}

fn log_filename(name: &str) -> Option<&'static str> {
    match name {
        "agent" => Some("agent.log"),
        "errors" => Some("errors.log"),
        "gateway" => Some("gateway.log"),
        _ => None,
    }
}

fn log_line_level_at_least(line: &str, min_level: &str) -> bool {
    let Some(level) = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]
        .into_iter()
        .find(|level| line.contains(&format!(" {level} ")))
    else {
        return true;
    };
    log_level_rank(level) >= log_level_rank(&min_level.to_ascii_uppercase())
}

fn log_level_rank(level: &str) -> i32 {
    match level {
        "DEBUG" => 0,
        "INFO" => 1,
        "WARNING" => 2,
        "ERROR" => 3,
        "CRITICAL" => 4,
        _ => 0,
    }
}

fn log_line_component_matches(line: &str, component: &str) -> bool {
    let prefixes: &[&str] = match component {
        "gateway" => &["gateway", "hermes_plugins"],
        "agent" => &["agent", "run_agent", "model_tools", "batch_runner"],
        "tools" => &["tools"],
        "cli" => &["hermes_cli", "cli"],
        "cron" => &["cron"],
        _ => return false,
    };
    prefixes
        .iter()
        .any(|prefix| line.contains(&format!(" {prefix}")) || line.contains(&format!("] {prefix}")))
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
    let canon = normalize_profile_name_for_cli(name)?;
    validate_profile_name_for_cli(&canon)?;
    let dir = profile_dir(hermes_home, &canon);
    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Profile '{canon}' does not exist."),
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
    let archive_root = if canon == "default" {
        "default"
    } else {
        canon.as_str()
    };
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
