#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "install-update-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_cli.config import (
            detect_install_method,
            recommended_update_command_for_method,
            stamp_install_method,
        )

        methods = ["nixos", "homebrew", "docker", "pip", "git", "unknown"]
        update_commands = {
            method: recommended_update_command_for_method(method) for method in methods
        }

        stamped = {}
        for method in ["git", "pip", "docker"]:
            stamp_install_method(method)
            stamped[method] = {
                "stamp": (home / ".install_method").read_text(encoding="utf-8"),
                "detected": detect_install_method(project_root=home),
            }

        cases = [
            {"name": "update_command_mapping", "commands": update_commands},
            {"name": "install_method_stamp", "stamped": stamped},
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
