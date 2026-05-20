from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "profile-migration-fixture.py"


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
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
