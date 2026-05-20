from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "profile-migration-fixture.py"


def _copy_policy(paths, root, ignore_cb, *, strip_root_runtime=False):
    results = []
    root = root.resolve()
    for relpath in paths:
        parts = relpath.split("/")
        directory = root
        ignored_at = None
        for index, part in enumerate(parts):
            ignored = set(ignore_cb(str(directory), [part]))
            if part in ignored:
                ignored_at = "/".join(parts[: index + 1])
                break
            directory = directory / part

        if ignored_at is not None:
            action = "excluded"
        elif strip_root_runtime and len(parts) == 1 and parts[0] in {
            "gateway.pid",
            "gateway_state.json",
            "processes.json",
        }:
            action = "stripped_after_copy"
        else:
            action = "kept"

        results.append(
            {
                "path": relpath,
                "action": action,
                "ignored_at": ignored_at,
            }
        )
    return results


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_cli import profiles

        names = [
            "Default",
            " Coder ",
            "Team_A",
            "",
            "bad/name",
            "tmp",
            "profile",
            "valid-01",
            "UPPER",
            "x" * 65,
        ]
        name_cases = []
        for value in names:
            try:
                normalized = profiles.normalize_profile_name(value)
                try:
                    profiles.validate_profile_name(normalized)
                    valid = True
                    error = None
                except Exception as exc:
                    valid = False
                    error = str(exc)
                name_cases.append(
                    {
                        "input": value,
                        "normalized": normalized,
                        "valid_after_normalize": valid,
                        "validation_error": error,
                    }
                )
            except Exception as exc:
                name_cases.append(
                    {
                        "input": value,
                        "normalized": None,
                        "valid_after_normalize": False,
                        "validation_error": str(exc),
                    }
                )

        root_names = [
            "hermes-agent",
            ".worktrees",
            "profiles",
            "bin",
            "node_modules",
            "config.yaml",
            "skills",
            "__pycache__",
            "cache.pyc",
            "cache.pyo",
            "runtime.sock",
            "scratch.tmp",
        ]
        nested_names = [
            "hermes-agent",
            "profiles",
            "config.yaml",
            "__pycache__",
            "runtime.sock",
            "scratch.tmp",
        ]
        default_ignore = profiles._clone_all_copytree_ignore(home)
        named_source = home / "profiles" / "coder"
        named_source.mkdir(parents=True)
        named_ignore = profiles._clone_all_copytree_ignore(named_source)

        export_ignore = profiles._default_export_ignore(home)
        export_names = [
            "hermes-agent",
            "state.db",
            "state.db-wal",
            ".env",
            "auth.json",
            "logs",
            "skills",
            "config.yaml",
            "package.json",
            "package-lock.json",
            "__pycache__",
            "runtime.sock",
            "scratch.tmp",
        ]
        tree_paths = [
            "config.yaml",
            ".env",
            "auth.json",
            "state.db",
            "sessions/session.jsonl",
            "skills/demo/SKILL.md",
            "plugins/demo/plugin.yaml",
            "cron/jobs.json",
            "gateway/telegram/state.json",
            "memories/MEMORY.md",
            "memories/USER.md",
            "tool_history/history.jsonl",
            "checkpoints/checkpoint.json",
            "trajectories/run.json",
            "exports/export.json",
            "logs/agent.log",
            "home/.ssh/config",
            "profile.yaml",
            "distribution.yaml",
            "SOUL.md",
            "hermes-agent/README.md",
            ".worktrees/w1/file",
            "profiles/coder/config.yaml",
            "bin/hermes",
            "node_modules/pkg/index.js",
            "skills/demo/__pycache__/cache.pyc",
            "skills/demo/cache.pyc",
            "skills/demo/cache.pyo",
            "gateway/runtime.sock",
            "workspace/scratch.tmp",
            "gateway.pid",
            "gateway_state.json",
            "processes.json",
            "package.json",
            "workspace/package-lock.json",
        ]
        named_export_ignore = lambda _directory, contents: {
            "auth.json",
            ".env",
        } & set(contents)

        cases = [
            {
                "name": "profile_constants",
                "profile_dirs": list(profiles._PROFILE_DIRS),
                "clone_config_files": list(profiles._CLONE_CONFIG_FILES),
                "clone_subdir_files": list(profiles._CLONE_SUBDIR_FILES),
                "clone_all_strip": list(profiles._CLONE_ALL_STRIP),
                "clone_all_default_exclude_root": sorted(
                    profiles._CLONE_ALL_DEFAULT_EXCLUDE_ROOT
                ),
                "default_export_exclude_root": sorted(
                    profiles._DEFAULT_EXPORT_EXCLUDE_ROOT
                ),
                "no_bundled_skills_marker": profiles.NO_BUNDLED_SKILLS_MARKER,
            },
            {"name": "profile_name_validation", "cases": name_cases},
            {
                "name": "clone_all_ignore",
                "root_names": root_names,
                "nested_names": nested_names,
                "default_root": sorted(default_ignore(str(home), root_names)),
                "default_nested": sorted(default_ignore(str(home / "nested"), nested_names)),
                "named_root": sorted(named_ignore(str(named_source), root_names)),
                "named_nested": sorted(
                    named_ignore(str(named_source / "nested"), nested_names)
                ),
            },
            {
                "name": "export_ignore",
                "root_names": export_names,
                "nested_names": nested_names,
                "root": sorted(export_ignore(home, export_names)),
                "nested": sorted(export_ignore(home / "nested", nested_names)),
            },
            {
                "name": "profile_tree_copy_policy",
                "paths": tree_paths,
                "clone_all_default": _copy_policy(
                    tree_paths,
                    home,
                    default_ignore,
                    strip_root_runtime=True,
                ),
                "clone_all_named": _copy_policy(
                    tree_paths,
                    named_source,
                    named_ignore,
                    strip_root_runtime=True,
                ),
                "export_default": _copy_policy(tree_paths, home, export_ignore),
                "export_named": _copy_policy(tree_paths, named_source, named_export_ignore),
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
