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
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
