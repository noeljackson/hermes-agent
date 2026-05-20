#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "config-defaults-fixture.py"


def _get(config: dict, path: str):
    cursor = config
    for part in path.split("."):
        cursor = cursor[part]
    return cursor


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from hermes_cli.config import DEFAULT_CONFIG

        paths = [
            "_config_version",
            "model",
            "toolsets",
            "agent.max_turns",
            "agent.gateway_timeout",
            "agent.restart_drain_timeout",
            "agent.api_max_retries",
            "agent.image_input_mode",
            "agent.disabled_toolsets",
            "terminal.backend",
            "terminal.cwd",
            "terminal.timeout",
            "terminal.docker_image",
            "terminal.container_cpu",
            "terminal.container_memory",
            "terminal.container_persistent",
            "browser.inactivity_timeout",
            "browser.command_timeout",
            "browser.allow_private_urls",
            "browser.engine",
            "checkpoints.enabled",
            "display.skin",
            "display.busy_input_mode",
            "memory.memory_enabled",
            "memory.provider",
            "security.redact_secrets",
            "security.allow_lazy_installs",
            "cron.wrap_response",
            "logging.level",
            "sessions.auto_prune",
            "updates.pre_update_backup",
            "lsp.enabled",
        ]
        cases = [
            {
                "name": "default_config_inventory",
                "top_level_keys": sorted(DEFAULT_CONFIG.keys()),
                "selected_values": {path: _get(DEFAULT_CONFIG, path) for path in paths},
            }
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
