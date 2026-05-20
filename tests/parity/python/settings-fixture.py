from __future__ import annotations

import contextlib
import io

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "settings-fixture.py"


def selected(config):
    model = config.get("model")
    if not isinstance(model, dict):
        model = {"raw": model}
    agent = config.get("agent")
    if not isinstance(agent, dict):
        agent = {}
    display = config.get("display")
    if not isinstance(display, dict):
        display = {}
    memory = config.get("memory")
    if not isinstance(memory, dict):
        memory = {}
    terminal = config.get("terminal")
    if not isinstance(terminal, dict):
        terminal = {}
    return {
        "top_level_keys": sorted(config.keys()),
        "config_version": config.get("_config_version"),
        "model": {
            "raw": model.get("raw"),
            "default": model.get("default"),
            "provider": model.get("provider"),
            "base_url": model.get("base_url"),
        },
        "agent": {
            "max_turns": agent.get("max_turns"),
            "system_prompt": agent.get("system_prompt"),
        },
        "display": {
            "skin": display.get("skin"),
            "tool_progress_command": display.get("tool_progress_command"),
        },
        "memory": {
            "provider": memory.get("provider"),
            "enabled": memory.get("enabled"),
        },
        "terminal": {
            "cwd": terminal.get("cwd"),
            "backend": terminal.get("backend"),
        },
    }


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        import yaml
        import hermes_cli.config as config_module

        defaults = config_module.load_config()

        user_config = {
            "model": {"provider": "openrouter", "default": "fake/model"},
            "agent": {"max_turns": 7},
            "display": {"skin": "mono"},
            "terminal": {"cwd": "/workspace"},
            "custom_section": {"kept": True},
        }
        (home / "config.yaml").write_text(
            yaml.safe_dump(user_config, sort_keys=True),
            encoding="utf-8",
        )
        config_module._LOAD_CONFIG_CACHE.clear()
        merged = config_module.load_config()

        legacy_config = {
            "provider": "legacy-provider",
            "base_url": "https://example.invalid/v1",
            "max_turns": 5,
        }
        (home / "config.yaml").write_text(
            yaml.safe_dump(legacy_config, sort_keys=True),
            encoding="utf-8",
        )
        config_module._LOAD_CONFIG_CACHE.clear()
        legacy = config_module.load_config()

        set_config_start = {
            "custom_providers": [
                {"name": "alpha", "api_key": "${ALPHA_KEY}"},
                {"name": "beta", "api_key": "${BETA_KEY}"},
            ],
            "terminal": {"backend": "local"},
        }
        (home / "config.yaml").write_text(
            yaml.safe_dump(set_config_start, sort_keys=True),
            encoding="utf-8",
        )
        config_module._LOAD_CONFIG_CACHE.clear()
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            config_module.set_config_value("custom_providers.1.api_key", "updated")
            config_module.set_config_value("custom_providers.0.enabled", "true")
            config_module.set_config_value("agent.max_turns", "12")
            config_module.set_config_value("terminal.timeout", "42")
            config_module.set_config_value("display.opacity", "0.75")
        set_config_raw = yaml.safe_load((home / "config.yaml").read_text(encoding="utf-8"))
        env_lines = []
        env_path = home / ".env"
        if env_path.exists():
            env_lines = [
                line.strip()
                for line in env_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]

    cases = [
        {"name": "defaults", "config": selected(defaults)},
        {"name": "deep_merge_overlay", "config": selected(merged)},
        {
            "name": "legacy_root_key_normalization",
            "config": selected(legacy),
        },
        {
            "name": "set_config_value_contract",
            "config": set_config_raw,
            "env_lines": env_lines,
            "stdout_markers": {
                "custom_provider_index": "custom_providers.1.api_key" in stdout.getvalue(),
                "agent_max_turns": "agent.max_turns = 12" in stdout.getvalue(),
                "terminal_timeout": "terminal.timeout = 42" in stdout.getvalue(),
                "display_float": "display.opacity = 0.75" in stdout.getvalue(),
            },
        },
    ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
