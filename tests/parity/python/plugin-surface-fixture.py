from __future__ import annotations

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
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
