#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "toolset-resolution-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        import toolsets

        names = toolsets.get_toolset_names()
        cases = [
            {
                "name": "toolset_inventory",
                "toolset_count": len(names),
                "names": names,
                "valid": {
                    name: toolsets.validate_toolset(name)
                    for name in ["web", "safe", "debugging", "hermes-cli", "all", "*", "missing"]
                },
            },
            {
                "name": "toolset_resolution",
                "resolved": {
                    name: toolsets.resolve_toolset(name)
                    for name in [
                        "web",
                        "safe",
                        "debugging",
                        "hermes-cli",
                        "hermes-discord",
                        "hermes-feishu",
                        "hermes-gateway",
                        "all",
                        "*",
                        "missing",
                    ]
                },
                "multiple": toolsets.resolve_multiple_toolsets(["web", "vision", "terminal"]),
            },
            {
                "name": "toolset_info",
                "info": {
                    name: toolsets.get_toolset_info(name)
                    for name in [
                        "web",
                        "safe",
                        "debugging",
                        "hermes-cli",
                        "hermes-gateway",
                        "missing",
                    ]
                },
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
