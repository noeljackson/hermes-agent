from __future__ import annotations

import zipfile
from pathlib import Path

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "backup-fixture.py"


def zip_with_members(path, members):
    with zipfile.ZipFile(path, "w") as zf:
        for member in members:
            if member.endswith("/"):
                zf.writestr(member, "")
            else:
                zf.writestr(member, "x")


def import_member_plan(member, prefix, home, secret_file_names):
    if member.endswith("/"):
        return {"member": member, "prefix": prefix, "action": "skip", "rel": ""}

    if prefix and member.startswith(prefix):
        rel = member[len(prefix) :]
    else:
        rel = member

    if not rel:
        return {"member": member, "prefix": prefix, "action": "skip", "rel": rel}

    target = home / rel
    try:
        restored_rel = target.resolve().relative_to(home.resolve()).as_posix()
    except ValueError:
        return {
            "member": member,
            "prefix": prefix,
            "action": "block",
            "rel": rel,
            "error": f"  {rel}: path traversal blocked",
        }

    return {
        "member": member,
        "prefix": prefix,
        "action": "restore",
        "rel": restored_rel,
        "secret": target.name in secret_file_names,
    }


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_cli.backup import (
            _SECRET_FILE_NAMES,
            _detect_prefix,
            _should_exclude,
            _validate_backup_zip,
        )

        exclusion_paths = [
            "config.yaml",
            ".env",
            "auth.json",
            "state.db",
            "profiles/demo/config.yaml",
            "profiles/demo/.env",
            "memories/MEMORY.md",
            "cron/jobs.json",
            "gateway_state.json",
            "gateway.pid",
            "cron.pid",
            "state.db-wal",
            "state.db-shm",
            "state.db-journal",
            "cache.pyc",
            "cache.pyo",
            "hermes-agent/run_agent.py",
            "profiles/demo/hermes-agent/run_agent.py",
            "backups/old.zip",
            "checkpoints/session/checkpoint.json",
            "node_modules/pkg/index.js",
            "skills/demo/SKILL.md",
            ".git/config",
            "__pycache__/module.pyc",
        ]
        exclusions = [
            {"path": path, "excluded": _should_exclude(Path(path))}
            for path in exclusion_paths
        ]

        prefix_cases = []
        for name, members in [
            ("root", ["config.yaml", ".env", "state.db"]),
            ("dot_hermes", [".hermes/config.yaml", ".hermes/.env"]),
            ("hermes", ["hermes/config.yaml", "hermes/state.db"]),
            ("profile_only", ["profiles/demo/config.yaml", "profiles/demo/.env"]),
            ("mixed", [".hermes/config.yaml", "other/.env"]),
            ("dirs_ignored", [".hermes/", ".hermes/config.yaml"]),
        ]:
            zip_path = home / f"{name}.zip"
            zip_with_members(zip_path, members)
            with zipfile.ZipFile(zip_path, "r") as zf:
                prefix_cases.append(
                    {
                        "name": name,
                        "members": members,
                        "prefix": _detect_prefix(zf),
                    }
                )

        validation_cases = []
        for name, members in [
            ("empty", []),
            ("no_markers", ["notes.txt", "profiles/demo/SOUL.md"]),
            ("config_root", ["config.yaml"]),
            ("env_wrapped", [".hermes/.env"]),
            ("state_nested", ["profiles/demo/state.db"]),
        ]:
            zip_path = home / f"validate-{name}.zip"
            zip_with_members(zip_path, members)
            with zipfile.ZipFile(zip_path, "r") as zf:
                ok, reason = _validate_backup_zip(zf)
                validation_cases.append(
                    {
                        "name": name,
                        "members": members,
                        "ok": ok,
                        "reason": reason,
                    }
                )

        import_member_cases = [
            import_member_plan(".hermes/config.yaml", ".hermes/", home, _SECRET_FILE_NAMES),
            import_member_plan(".hermes/.env", ".hermes/", home, _SECRET_FILE_NAMES),
            import_member_plan(".hermes/auth.json", ".hermes/", home, _SECRET_FILE_NAMES),
            import_member_plan(".hermes/state.db", ".hermes/", home, _SECRET_FILE_NAMES),
            import_member_plan(
                ".hermes/profiles/demo/auth.json",
                ".hermes/",
                home,
                _SECRET_FILE_NAMES,
            ),
            import_member_plan(
                ".hermes/profiles/demo/SOUL.md",
                ".hermes/",
                home,
                _SECRET_FILE_NAMES,
            ),
            import_member_plan(".hermes/../escape.txt", ".hermes/", home, _SECRET_FILE_NAMES),
            import_member_plan("../escape.txt", "", home, _SECRET_FILE_NAMES),
            import_member_plan("/tmp/escape.txt", "", home, _SECRET_FILE_NAMES),
            import_member_plan("safe/../config.yaml", "", home, _SECRET_FILE_NAMES),
            import_member_plan(".hermes/", ".hermes/", home, _SECRET_FILE_NAMES),
        ]

        cases = [
            {
                "name": "full_backup_exclusion_policy",
                "paths": exclusions,
                "secret_file_names": sorted(_SECRET_FILE_NAMES),
            },
            {
                "name": "import_prefix_detection",
                "archives": prefix_cases,
            },
            {
                "name": "import_validation",
                "archives": validation_cases,
            },
            {
                "name": "import_member_planning",
                "members": import_member_cases,
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
