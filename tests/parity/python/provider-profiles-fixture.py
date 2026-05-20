from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "provider-profiles-fixture.py"


def _fixed_temperature(value):
    try:
        from providers import OMIT_TEMPERATURE

        if value is OMIT_TEMPERATURE:
            return "omit"
    except Exception:
        pass
    return value


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from providers import get_provider_profile, list_providers

        profiles = []
        for profile in sorted(list_providers(), key=lambda p: p.name):
            profiles.append(
                {
                    "name": profile.name,
                    "aliases": sorted(profile.aliases),
                    "api_mode": profile.api_mode,
                    "display_name": profile.display_name,
                    "env_vars": list(profile.env_vars),
                    "base_url": profile.base_url,
                    "auth_type": profile.auth_type,
                    "supports_health_check": profile.supports_health_check,
                    "fallback_model_count": len(profile.fallback_models),
                    "default_max_tokens": profile.default_max_tokens,
                    "fixed_temperature": _fixed_temperature(profile.fixed_temperature),
                    "default_header_keys": sorted(profile.default_headers.keys()),
                }
            )

        alias_cases = {
            "anthropic": "anthropic",
            "claude": "anthropic",
            "openrouter": "openrouter",
            "or": "openrouter",
            "openai-codex": "openai-codex",
        }
        aliases = {}
        for alias, expected in alias_cases.items():
            profile = get_provider_profile(alias)
            aliases[alias] = profile.name if profile else None
            assert aliases[alias] == expected

        cases = [
            {
                "name": "provider_inventory",
                "provider_count": len(profiles),
                "profiles": profiles,
            },
            {
                "name": "provider_alias_resolution",
                "aliases": aliases,
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
