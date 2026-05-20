from __future__ import annotations

import os
from pathlib import Path

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "plugin-surface-fixture.py"


CONTROLLED_FILES = {
    "flat-demo/plugin.yaml": """
name: flat-demo
version: 1.2.3
description: Flat demo plugin.
author: Parity
requires_env:
  - FLAT_TOKEN
  - name: FLAT_OPTIONAL
    optional: true
provides_tools:
  - flat_tool
provides_hooks:
  - pre_tool_call
""".lstrip(),
    "flat-demo/__init__.py": "def register(ctx): pass\n",
    "image_gen/openai/plugin.yaml": """
name: openai
kind: backend
version: 0.1.0
description: Image backend.
""".lstrip(),
    "image_gen/openai/__init__.py": "def register(ctx): pass\n",
    "memory-auto/plugin.yaml": """
name: memory-auto
description: Auto-detected memory provider.
""".lstrip(),
    "memory-auto/__init__.py": "from agent.memory_provider import MemoryProvider\n",
    "provider-auto/plugin.yaml": """
name: provider-auto
description: Auto-detected model provider.
""".lstrip(),
    "provider-auto/__init__.py": "from providers import register_provider, ProviderProfile\n",
    "bad-kind/plugin.yaml": """
name: bad-kind
kind: surprising
description: Invalid kind falls back.
""".lstrip(),
    "bad-kind/__init__.py": "def register(ctx): pass\n",
    "too/deep/plugin/plugin.yaml": "name: too-deep\n",
    "skip-me/plugin.yaml": "name: skip-me\n",
}

POLICY_FILES = {
    "bundled/bundled-standalone/plugin.yaml": """
name: bundled-standalone
kind: standalone
description: Bundled standalone stays opt-in.
""".lstrip(),
    "bundled/bundled-standalone/__init__.py": "def register(ctx): ctx.register_hook('pre_tool_call', lambda **kw: None)\n",
    "bundled/image_gen/auto-backend/plugin.yaml": """
name: auto-backend
kind: backend
description: Bundled backend auto-loads.
""".lstrip(),
    "bundled/image_gen/auto-backend/__init__.py": "def register(ctx): ctx.register_hook('post_tool_call', lambda **kw: None)\n",
    "bundled/platforms/auto-platform/plugin.yaml": """
name: auto-platform
kind: platform
description: Bundled platform auto-loads.
""".lstrip(),
    "bundled/platforms/auto-platform/__init__.py": "def register(ctx): ctx.register_hook('post_llm_call', lambda **kw: None)\n",
    "bundled/model-providers/skip-provider/plugin.yaml": """
name: skip-provider
kind: model-provider
description: Model providers are discovered elsewhere.
""".lstrip(),
    "bundled/model-providers/skip-provider/__init__.py": "raise RuntimeError('model provider should not be loaded by PluginManager')\n",
    "home/plugins/enabled-user/plugin.yaml": """
name: enabled-user
kind: standalone
description: Explicitly enabled user plugin.
""".lstrip(),
    "home/plugins/enabled-user/__init__.py": "def register(ctx): ctx.register_command('enabled-user-cmd', lambda raw: 'ok')\n",
    "home/plugins/disabled-user/plugin.yaml": """
name: disabled-user
kind: standalone
description: Disabled user plugin.
""".lstrip(),
    "home/plugins/disabled-user/__init__.py": "raise RuntimeError('disabled plugin should not load')\n",
    "home/plugins/user-backend/plugin.yaml": """
name: user-backend
kind: backend
description: User backend remains opt-in.
""".lstrip(),
    "home/plugins/user-backend/__init__.py": "raise RuntimeError('user backend should not auto-load')\n",
    "project/.hermes/plugins/enabled-user/plugin.yaml": """
name: enabled-user
kind: standalone
description: Project plugin overrides user plugin when project plugins are enabled.
""".lstrip(),
    "project/.hermes/plugins/enabled-user/__init__.py": "def register(ctx): ctx.register_command('project-user-cmd', lambda raw: 'ok')\n",
}

MEMORY_FILES = {
    "bundled/alpha/plugin.yaml": "name: alpha\ndescription: Bundled alpha provider.\n",
    "bundled/alpha/__init__.py": "class AlphaMemoryProvider: pass\n",
    "bundled/collision/plugin.yaml": "name: collision\ndescription: Bundled collision wins.\n",
    "bundled/collision/__init__.py": "class CollisionMemoryProvider: pass\n",
    "bundled/no_init/plugin.yaml": "name: no_init\ndescription: Missing init is ignored.\n",
    "bundled/_private/__init__.py": "class PrivateMemoryProvider: pass\n",
    "bundled/.hidden/__init__.py": "class HiddenMemoryProvider: pass\n",
    "home/plugins/collision/plugin.yaml": "name: collision\ndescription: User collision loses.\n",
    "home/plugins/collision/__init__.py": "from agent.memory_provider import MemoryProvider\n",
    "home/plugins/user-memory/plugin.yaml": "name: user-memory\ndescription: User memory provider.\n",
    "home/plugins/user-memory/__init__.py": "from agent.memory_provider import MemoryProvider\n",
    "home/plugins/register-memory/plugin.yaml": "name: register-memory\ndescription: Register memory provider.\n",
    "home/plugins/register-memory/__init__.py": "def register_memory_provider(ctx): pass\n",
    "home/plugins/not-memory/plugin.yaml": "name: not-memory\ndescription: Not a memory provider.\n",
    "home/plugins/not-memory/__init__.py": "def register(ctx): pass\n",
    "home/plugins/no-init/plugin.yaml": "name: no-init\ndescription: Missing init is ignored.\n",
}


def write_files(root: Path, files: dict[str, str]) -> None:
    for rel, content in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def manifest_dict(manifest, root: Path) -> dict:
    data = {
        "name": manifest.name,
        "version": manifest.version,
        "description": manifest.description,
        "author": manifest.author,
        "requires_env": manifest.requires_env,
        "provides_tools": manifest.provides_tools,
        "provides_hooks": manifest.provides_hooks,
        "source": manifest.source,
        "path": str(Path(manifest.path).relative_to(root)) if manifest.path else "",
        "kind": manifest.kind,
        "key": manifest.key,
    }
    return data


def loaded_plugin_dict(key: str, loaded) -> dict:
    return {
        "key": key,
        "name": loaded.manifest.name,
        "kind": loaded.manifest.kind,
        "source": loaded.manifest.source,
        "enabled": loaded.enabled,
        "error": loaded.error,
        "hooks_registered": sorted(loaded.hooks_registered),
        "commands_registered": sorted(loaded.commands_registered),
    }


def memory_provider_dirs(memory_module, root: Path) -> list[dict]:
    return [
        {"name": name, "path": str(path.relative_to(root))}
        for name, path in memory_module._iter_provider_dirs()
    ]


def provider_registry_selection_case() -> dict:
    from agent.browser_provider import BrowserProvider
    from agent.image_gen_provider import ImageGenProvider
    from agent.web_search_provider import WebSearchProvider
    from agent import browser_registry, image_gen_registry, web_search_registry

    class FakeImageProvider(ImageGenProvider):
        def __init__(self, name: str, available: bool = True):
            self._name = name
            self._available = available

        @property
        def name(self) -> str:
            return self._name

        def is_available(self) -> bool:
            return self._available

        def generate(self, prompt: str, aspect_ratio: str = "landscape", **kwargs):
            return {"success": True, "provider": self.name, "prompt": prompt}

    class FakeWebProvider(WebSearchProvider):
        def __init__(
            self,
            name: str,
            *,
            available: bool = True,
            search: bool = True,
            extract: bool = False,
            crawl: bool = False,
        ):
            self._name = name
            self._available = available
            self._search = search
            self._extract = extract
            self._crawl = crawl

        @property
        def name(self) -> str:
            return self._name

        def is_available(self) -> bool:
            return self._available

        def supports_search(self) -> bool:
            return self._search

        def supports_extract(self) -> bool:
            return self._extract

        def supports_crawl(self) -> bool:
            return self._crawl

    class FakeBrowserProvider(BrowserProvider):
        def __init__(self, name: str, available: bool = True):
            self._name = name
            self._available = available

        @property
        def name(self) -> str:
            return self._name

        def is_available(self) -> bool:
            return self._available

        def create_session(self, task_id: str):
            return {"session_name": task_id, "bb_session_id": task_id, "cdp_url": "", "features": {}}

        def close_session(self, session_id: str) -> bool:
            return True

        def emergency_cleanup(self, session_id: str) -> None:
            return None

    def image_active(providers: list[dict], configured):
        image_gen_registry._reset_for_tests()
        for spec in providers:
            image_gen_registry.register_provider(
                FakeImageProvider(spec["name"], spec.get("available", True))
            )

        import hermes_cli.config as config_module

        original_load_config = config_module.load_config
        try:
            if configured is None:
                config_module.load_config = lambda: {"image_gen": {"provider": ""}}
            else:
                config_module.load_config = lambda: {"image_gen": {"provider": configured}}
            active = image_gen_registry.get_active_provider()
            return active.name if active else None
        finally:
            config_module.load_config = original_load_config
            image_gen_registry._reset_for_tests()

    def web_active(providers: list[dict], configured, capability: str):
        web_search_registry._reset_for_tests()
        for spec in providers:
            web_search_registry.register_provider(
                FakeWebProvider(
                    spec["name"],
                    available=spec.get("available", True),
                    search=spec.get("search", True),
                    extract=spec.get("extract", False),
                    crawl=spec.get("crawl", False),
                )
            )
        try:
            active = web_search_registry._resolve(configured, capability=capability)
            return active.name if active else None
        finally:
            web_search_registry._reset_for_tests()

    def browser_active(providers: list[dict], configured):
        browser_registry._reset_for_tests()
        for spec in providers:
            browser_registry.register_provider(
                FakeBrowserProvider(spec["name"], spec.get("available", True))
            )
        try:
            active = browser_registry._resolve(configured)
            return active.name if active else None
        finally:
            browser_registry._reset_for_tests()

    image_lookup_providers = [
        {"name": "zeta", "available": True},
        {"name": "fal", "available": True},
        {"name": "alpha", "available": False},
    ]
    image_gen_registry._reset_for_tests()
    for spec in image_lookup_providers:
        image_gen_registry.register_provider(
            FakeImageProvider(spec["name"], spec["available"])
        )
    image_lookup = {
        "get_provider_inputs": {"trimmed": " fal ", "missing": "missing", "non_string": 123},
        "list": [provider.name for provider in image_gen_registry.list_providers()],
        "get_provider": {
            "trimmed": (image_gen_registry.get_provider(" fal ") or None).name,
            "missing": None if image_gen_registry.get_provider("missing") is None else "unexpected",
            "non_string": None if image_gen_registry.get_provider(123) is None else "unexpected",
        },
    }
    image_gen_registry._reset_for_tests()

    web_lookup_providers = [
        {"name": "tavily", "available": True, "search": True, "extract": True, "crawl": True},
        {"name": "brave-free", "available": True, "search": True, "extract": False, "crawl": False},
        {"name": "exa", "available": False, "search": True, "extract": True, "crawl": False},
    ]
    web_search_registry._reset_for_tests()
    for spec in web_lookup_providers:
        web_search_registry.register_provider(
            FakeWebProvider(
                spec["name"],
                available=spec["available"],
                search=spec["search"],
                extract=spec["extract"],
                crawl=spec["crawl"],
            )
        )
    web_lookup = {
        "get_provider_inputs": {"trimmed": " tavily ", "missing": "missing", "non_string": 123},
        "legacy_preference": list(web_search_registry._LEGACY_PREFERENCE),
        "list": [provider.name for provider in web_search_registry.list_providers()],
        "get_provider": {
            "trimmed": (web_search_registry.get_provider(" tavily ") or None).name,
            "missing": None if web_search_registry.get_provider("missing") is None else "unexpected",
            "non_string": None if web_search_registry.get_provider(123) is None else "unexpected",
        },
    }
    web_search_registry._reset_for_tests()

    browser_lookup_providers = [
        {"name": "firecrawl", "available": True},
        {"name": "browserbase", "available": True},
        {"name": "browser-use", "available": False},
    ]
    browser_registry._reset_for_tests()
    for spec in browser_lookup_providers:
        browser_registry.register_provider(FakeBrowserProvider(spec["name"], spec["available"]))
    browser_lookup = {
        "get_provider_inputs": {"trimmed": " browserbase ", "missing": "missing", "non_string": 123},
        "legacy_preference": list(browser_registry._LEGACY_PREFERENCE),
        "list": [provider.name for provider in browser_registry.list_providers()],
        "get_provider": {
            "trimmed": (browser_registry.get_provider(" browserbase ") or None).name,
            "missing": None if browser_registry.get_provider("missing") is None else "unexpected",
            "non_string": None if browser_registry.get_provider(123) is None else "unexpected",
        },
    }
    browser_registry._reset_for_tests()

    image_cases = [
        {
            "label": "explicit_unavailable_wins",
            "providers": [{"name": "alpha", "available": False}, {"name": "fal", "available": True}],
            "configured": "alpha",
        },
        {
            "label": "configured_missing_falls_back",
            "providers": [{"name": "fal", "available": True}, {"name": "zeta", "available": False}],
            "configured": "missing",
        },
        {
            "label": "single_available_shortcut",
            "providers": [{"name": "alpha", "available": False}, {"name": "zeta", "available": True}],
            "configured": None,
        },
        {
            "label": "fal_legacy_preference",
            "providers": [{"name": "fal", "available": True}, {"name": "zeta", "available": True}],
            "configured": None,
        },
        {
            "label": "no_available_provider",
            "providers": [{"name": "fal", "available": False}, {"name": "zeta", "available": False}],
            "configured": None,
        },
    ]
    for case in image_cases:
        case["active"] = image_active(case["providers"], case["configured"])

    web_cases = [
        {
            "label": "explicit_unavailable_capable_wins",
            "providers": [{"name": "exa", "available": False, "search": True, "extract": True, "crawl": False}],
            "configured": "exa",
            "capability": "extract",
        },
        {
            "label": "configured_incapable_falls_back",
            "providers": [
                {"name": "brave-free", "available": True, "search": True, "extract": False, "crawl": False},
                {"name": "tavily", "available": True, "search": True, "extract": True, "crawl": True},
            ],
            "configured": "brave-free",
            "capability": "extract",
        },
        {
            "label": "single_eligible_shortcut",
            "providers": [
                {"name": "alpha", "available": True, "search": True, "extract": False, "crawl": False},
                {"name": "zeta", "available": True, "search": False, "extract": True, "crawl": False},
            ],
            "configured": None,
            "capability": "extract",
        },
        {
            "label": "legacy_preference_order",
            "providers": [
                {"name": "tavily", "available": True, "search": True, "extract": True, "crawl": True},
                {"name": "firecrawl", "available": True, "search": True, "extract": True, "crawl": True},
                {"name": "brave-free", "available": True, "search": True, "extract": False, "crawl": False},
            ],
            "configured": None,
            "capability": "search",
        },
        {
            "label": "crawl_capability_filter",
            "providers": [
                {"name": "brave-free", "available": True, "search": True, "extract": False, "crawl": False},
                {"name": "tavily", "available": True, "search": True, "extract": True, "crawl": True},
            ],
            "configured": None,
            "capability": "crawl",
        },
    ]
    for case in web_cases:
        case["active"] = web_active(case["providers"], case["configured"], case["capability"])

    browser_cases = [
        {
            "label": "explicit_local_disables_cloud",
            "providers": [{"name": "browser-use", "available": True}],
            "configured": "local",
        },
        {
            "label": "explicit_unavailable_wins",
            "providers": [{"name": "browserbase", "available": False}],
            "configured": "browserbase",
        },
        {
            "label": "legacy_preference_order",
            "providers": [
                {"name": "browserbase", "available": True},
                {"name": "browser-use", "available": True},
            ],
            "configured": None,
        },
        {
            "label": "firecrawl_not_auto_selected",
            "providers": [{"name": "firecrawl", "available": True}],
            "configured": None,
        },
        {
            "label": "missing_configured_falls_back",
            "providers": [{"name": "browserbase", "available": True}],
            "configured": "missing",
        },
    ]
    for case in browser_cases:
        case["active"] = browser_active(case["providers"], case["configured"])

    return {
        "name": "provider_registry_selection",
        "image_gen": {**image_lookup, "providers": image_lookup_providers, "resolution_cases": image_cases},
        "web": {**web_lookup, "providers": web_lookup_providers, "resolution_cases": web_cases},
        "browser": {**browser_lookup, "providers": browser_lookup_providers, "resolution_cases": browser_cases},
    }


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_cli import plugins

        root = home / "plugins-fixture"
        write_files(root, CONTROLLED_FILES)

        manager = plugins.PluginManager()
        manifests = manager._scan_directory(
            root,
            source="user",
            skip_names={"skip-me"},
        )

        policy_root = home / "policy-fixture"
        write_files(policy_root, POLICY_FILES)
        original_bundled_dir = plugins.get_bundled_plugins_dir
        original_home = plugins.get_hermes_home
        original_enabled = plugins._get_enabled_plugins
        original_disabled = plugins._get_disabled_plugins
        original_env_enabled = plugins._env_enabled
        original_cwd = Path.cwd()
        try:
            plugins.get_bundled_plugins_dir = lambda: policy_root / "bundled"
            plugins.get_hermes_home = lambda: policy_root / "home"
            plugins._get_enabled_plugins = lambda: {"enabled-user"}
            plugins._get_disabled_plugins = lambda: {"disabled-user"}
            plugins._env_enabled = lambda name: name == "HERMES_ENABLE_PROJECT_PLUGINS"
            (policy_root / "project").mkdir(parents=True, exist_ok=True)

            os.chdir(policy_root / "project")
            policy_manager = plugins.PluginManager()
            policy_manager.discover_and_load(force=True)
            loaded_plugins = [
                loaded_plugin_dict(key, loaded)
                for key, loaded in sorted(policy_manager._plugins.items())
            ]
            registered_hooks = sorted(policy_manager._hooks.keys())
            registered_commands = sorted(policy_manager._plugin_commands.keys())
        finally:
            os.chdir(original_cwd)
            plugins.get_bundled_plugins_dir = original_bundled_dir
            plugins.get_hermes_home = original_home
            plugins._get_enabled_plugins = original_enabled
            plugins._get_disabled_plugins = original_disabled
            plugins._env_enabled = original_env_enabled

        from plugins import memory as memory_plugins

        memory_root = home / "memory-fixture"
        write_files(memory_root, MEMORY_FILES)
        original_memory_plugins_dir = memory_plugins._MEMORY_PLUGINS_DIR
        original_memory_user_dir = memory_plugins._get_user_plugins_dir
        try:
            memory_plugins._MEMORY_PLUGINS_DIR = memory_root / "bundled"
            memory_plugins._get_user_plugins_dir = lambda: memory_root / "home" / "plugins"
            memory_dirs = memory_provider_dirs(memory_plugins, memory_root)
            find_results = {
                name: (
                    str(found.relative_to(memory_root))
                    if (found := memory_plugins.find_provider_dir(name))
                    else None
                )
                for name in [
                    "alpha",
                    "collision",
                    "user-memory",
                    "register-memory",
                    "not-memory",
                    "missing",
                ]
            }
            heuristics = {
                name: memory_plugins._is_memory_provider_dir(
                    memory_root / "home" / "plugins" / name
                )
                for name in ["user-memory", "register-memory", "not-memory", "no-init"]
            }
        finally:
            memory_plugins._MEMORY_PLUGINS_DIR = original_memory_plugins_dir
            memory_plugins._get_user_plugins_dir = original_memory_user_dir

        cases = [
            {
                "name": "plugin_boundary_constants",
                "valid_hooks": sorted(plugins.VALID_HOOKS),
                "valid_kinds": sorted(plugins._VALID_PLUGIN_KINDS),
                "entry_points_group": plugins.ENTRY_POINTS_GROUP,
            },
            {
                "name": "controlled_manifest_scan",
                "files": CONTROLLED_FILES,
                "skip_names": ["skip-me"],
                "manifests": [manifest_dict(manifest, root) for manifest in manifests],
            },
            {
                "name": "load_policy",
                "files": POLICY_FILES,
                "enabled": ["enabled-user"],
                "disabled": ["disabled-user"],
                "plugins": loaded_plugins,
                "registered_hooks": registered_hooks,
                "registered_commands": registered_commands,
            },
            {
                "name": "memory_provider_discovery",
                "files": MEMORY_FILES,
                "provider_dirs": memory_dirs,
                "find_provider_dir": find_results,
                "heuristics": heuristics,
            },
            provider_registry_selection_case(),
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
