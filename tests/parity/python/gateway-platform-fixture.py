#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "gateway-platform-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from gateway.config import HomeChannel, Platform, PlatformConfig, SessionResetPolicy

        platforms = [platform.value for platform in Platform.__members__.values()]
        default_platform_config = PlatformConfig().to_dict()
        default_reset_policy = SessionResetPolicy().to_dict()
        home = HomeChannel(
            platform=Platform.TELEGRAM,
            chat_id="chat-1",
            name="Parity Chat",
            thread_id="topic-1",
        ).to_dict()
        parsed_platform = Platform("telegram").value

        cases = [
            {
                "name": "builtin_platform_inventory",
                "platform_count": len(platforms),
                "platforms": platforms,
                "parsed_platform": parsed_platform,
            },
            {
                "name": "platform_config_defaults",
                "config": default_platform_config,
                "reset_policy": default_reset_policy,
                "home_channel": home,
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
