from __future__ import annotations

import os
from pathlib import Path

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "terminal-backend-fixture.py"


TERMINAL_ENV_KEYS = [
    "TERMINAL_ENV",
    "TERMINAL_CWD",
    "TERMINAL_DOCKER_MOUNT_CWD_TO_WORKSPACE",
    "TERMINAL_DOCKER_IMAGE",
    "TERMINAL_DOCKER_FORWARD_ENV",
    "TERMINAL_DOCKER_ENV",
    "TERMINAL_DOCKER_VOLUMES",
    "TERMINAL_DOCKER_EXTRA_ARGS",
    "TERMINAL_SSH_HOST",
    "TERMINAL_SSH_USER",
    "TERMINAL_SSH_PORT",
    "TERMINAL_SSH_KEY",
    "TERMINAL_TIMEOUT",
    "TERMINAL_CONTAINER_CPU",
    "TERMINAL_CONTAINER_MEMORY",
    "TERMINAL_CONTAINER_DISK",
    "TERMINAL_CONTAINER_PERSISTENT",
    "TERMINAL_LOCAL_PERSISTENT",
    "TERMINAL_PERSISTENT_SHELL",
    "TERMINAL_SSH_PERSISTENT",
    "TERMINAL_MODAL_MODE",
]


def selected(config):
    return {
        "env_type": config.get("env_type"),
        "cwd": config.get("cwd"),
        "host_cwd": config.get("host_cwd"),
        "timeout": config.get("timeout"),
        "docker_image": config.get("docker_image"),
        "docker_mount_cwd_to_workspace": config.get("docker_mount_cwd_to_workspace"),
        "docker_forward_env": config.get("docker_forward_env"),
        "docker_env": config.get("docker_env"),
        "docker_volumes": config.get("docker_volumes"),
        "docker_extra_args": config.get("docker_extra_args"),
        "ssh_host": config.get("ssh_host"),
        "ssh_user": config.get("ssh_user"),
        "ssh_port": config.get("ssh_port"),
        "ssh_key_present": bool(config.get("ssh_key")),
        "local_persistent": config.get("local_persistent"),
        "ssh_persistent": config.get("ssh_persistent"),
        "modal_mode": config.get("modal_mode"),
        "container_cpu": config.get("container_cpu"),
        "container_memory": config.get("container_memory"),
        "container_disk": config.get("container_disk"),
        "container_persistent": config.get("container_persistent"),
    }


def with_env(values):
    for key in TERMINAL_ENV_KEYS:
        os.environ.pop(key, None)
    os.environ.update(values)
    from tools.terminal_tool import _get_env_config

    return selected(_get_env_config())


def error_with_env(values):
    for key in TERMINAL_ENV_KEYS:
        os.environ.pop(key, None)
    os.environ.update(values)
    from tools.terminal_tool import _get_env_config

    try:
        return {"ok": True, "config": selected(_get_env_config())}
    except Exception as exc:
        return {"ok": False, "error": str(exc)}


def remote_backend_contracts(home: Path):
    from tools.environments import base as base_env
    from tools.environments import docker as docker_env
    from tools.environments.file_sync import (
        quoted_mkdir_command,
        quoted_rm_command,
        unique_parent_dirs,
    )
    from tools.environments.ssh import SSHEnvironment
    from tools.environments import modal as modal_env
    from tools.environments import modal_utils

    class FakeUuid:
        def __init__(self, hex_value: str):
            self.hex = hex_value

    class FixtureEnvironment(base_env.BaseEnvironment):
        def _run_bash(self, *args, **kwargs):
            raise NotImplementedError

        def cleanup(self):
            pass

    base = FixtureEnvironment.__new__(FixtureEnvironment)
    base.cwd = "/tmp/start"
    base.timeout = 30
    base.env = {}
    base._session_id = "abc123def456"
    base._snapshot_path = "/tmp/snap path.sh"
    base._cwd_file = "/tmp/cwd path.txt"
    base._cwd_marker = base_env._cwd_marker(base._session_id)
    base._snapshot_ready = True

    original_base_uuid = base_env.uuid.uuid4
    original_modal_uuid = modal_utils.uuid.uuid4
    try:
        base_env.uuid.uuid4 = lambda: FakeUuid("feedface1234567890")
        modal_values = iter(
            [
                FakeUuid("deadbeef"),
                FakeUuid("cafebabe"),
            ]
        )
        modal_utils.uuid.uuid4 = lambda: next(modal_values)
        base_modal_contracts = {
            "cwd_marker": base_env._cwd_marker("abc123"),
            "quote_cwd": [
                {"cwd": "~", "quoted": base_env.BaseEnvironment._quote_cwd_for_cd("~")},
                {"cwd": "~/", "quoted": base_env.BaseEnvironment._quote_cwd_for_cd("~/")},
                {
                    "cwd": "~/project dir",
                    "quoted": base_env.BaseEnvironment._quote_cwd_for_cd("~/project dir"),
                },
                {
                    "cwd": "/tmp/path with spaces",
                    "quoted": base_env.BaseEnvironment._quote_cwd_for_cd("/tmp/path with spaces"),
                },
                {
                    "cwd": "-dash",
                    "quoted": base_env.BaseEnvironment._quote_cwd_for_cd("-dash"),
                },
            ],
            "wrapped_snapshot_ready": base._wrap_command("printf 'hi'", "~/work dir"),
            "embedded_stdin": base_env.BaseEnvironment._embed_stdin_heredoc(
                "cat", "hello\nworld"
            ),
            "modal_stdin": modal_utils.wrap_modal_stdin_heredoc(
                "cat", "first HERMES_EOF_deadbeef marker"
            ),
            "modal_sudo": modal_utils.wrap_modal_sudo_pipe(
                "sudo -S true", "pa ss\n"
            ),
        }
    finally:
        base_env.uuid.uuid4 = original_base_uuid
        modal_utils.uuid.uuid4 = original_modal_uuid

    ssh = SSHEnvironment.__new__(SSHEnvironment)
    ssh.host = "ssh.example.invalid"
    ssh.user = "hermes"
    ssh.port = 2222
    ssh.key_path = "/tmp/fake key"
    ssh.control_socket = Path("/tmp/hermes-ssh/fixture.sock")

    modal_store = home / "modal_snapshots.json"
    original_snapshot_store = modal_env._SNAPSHOT_STORE
    try:
        modal_env._SNAPSHOT_STORE = modal_store
        modal_env._save_snapshots(
            {
                "direct:task-a": "snap-direct",
                "task-b": "snap-legacy",
                "direct:task-c": "snap-current",
                "task-c": "snap-old",
            }
        )
        modal_snapshot_cases = {
            "direct_key": modal_env._direct_snapshot_key("task-a"),
            "restore_direct": modal_env._get_snapshot_restore_candidate("task-a"),
            "restore_legacy": modal_env._get_snapshot_restore_candidate("task-b"),
            "restore_missing": modal_env._get_snapshot_restore_candidate("missing"),
        }
        modal_env._delete_direct_snapshot("task-c", "snap-current")
        modal_snapshot_cases["after_delete_specific"] = modal_env._load_snapshots()
        modal_env._store_direct_snapshot("task-d", "snap-new")
        modal_snapshot_cases["after_store_direct"] = modal_env._load_snapshots()
    finally:
        modal_env._SNAPSHOT_STORE = original_snapshot_store

    return {
        "name": "remote_backend_contracts",
        "docker": {
            "forward_env": docker_env._normalize_forward_env_names(
                [" SSH_AUTH_SOCK ", "bad-name!", "", "SSH_AUTH_SOCK", 7, "CI"]
            ),
            "env_dict": docker_env._normalize_env_dict(
                {
                    " CI ": "1",
                    "COUNT": 3,
                    "FLAG": True,
                    "bad-name!": "drop",
                    "COMPLEX": {"drop": True},
                }
            ),
            "security_args_root": docker_env._build_security_args(False),
            "security_args_host_user": docker_env._build_security_args(True),
        },
        "ssh": {
            "base_command": ssh._build_ssh_command(),
            "extra_args_command": ssh._build_ssh_command(["-tt"]),
        },
        "file_sync": {
            "quoted_mkdir": quoted_mkdir_command(
                ["/home/hermes/.hermes", "/tmp/path with spaces"]
            ),
            "quoted_rm": quoted_rm_command(
                ["/home/hermes/.hermes/a.txt", "/tmp/path with spaces/b.txt"]
            ),
            "unique_parent_dirs": unique_parent_dirs(
                [
                    ("/host/a", "/remote/one/a.txt"),
                    ("/host/b", "/remote/two/b.txt"),
                    ("/host/c", "/remote/one/c.txt"),
                ]
            ),
        },
        "modal_snapshots": modal_snapshot_cases,
        "base_modal": base_modal_contracts,
    }


def safety_helper_contracts():
    from tools.terminal_tool import (
        _foreground_background_guidance,
        _resolve_notification_flag_conflict,
        _rewrite_compound_background,
        _transform_sudo_command,
    )

    rewrite_inputs = [
        "python server.py &",
        "npm install && npm run dev &",
        "false || python -m http.server &",
        "npm install && { npm run dev & }",
        "printf 'a && b &'",
        "echo hi &> out.txt",
    ]
    guidance_inputs = [
        "python -m http.server",
        "python -m http.server --help",
        "nohup python server.py",
        "sleep 1 &",
        "git commit -m 'run nohup now'",
        "npm run dev",
    ]

    notification_inputs = [
        {
            "background": True,
            "notify_on_complete": True,
            "watch_patterns": ["READY"],
        },
        {
            "background": False,
            "notify_on_complete": True,
            "watch_patterns": ["READY"],
        },
        {
            "background": True,
            "notify_on_complete": False,
            "watch_patterns": ["READY"],
        },
    ]

    original_sudo = os.environ.get("SUDO_PASSWORD")
    original_interactive = os.environ.get("HERMES_INTERACTIVE")
    try:
        os.environ.pop("HERMES_INTERACTIVE", None)
        sudo_cases = []
        for password, command in [
            (None, "sudo ls /root"),
            ("pa ss", "sudo ls /root && echo done"),
            ("pa ss", "echo sudo"),
            ("pa ss", "FOO=1 sudo whoami"),
        ]:
            if password is None:
                os.environ.pop("SUDO_PASSWORD", None)
            else:
                os.environ["SUDO_PASSWORD"] = password
            transformed, stdin = _transform_sudo_command(command)
            sudo_cases.append(
                {
                    "command": command,
                    "password_present": password is not None,
                    "transformed": transformed,
                    "sudo_stdin": stdin,
                }
            )
    finally:
        if original_sudo is None:
            os.environ.pop("SUDO_PASSWORD", None)
        else:
            os.environ["SUDO_PASSWORD"] = original_sudo
        if original_interactive is None:
            os.environ.pop("HERMES_INTERACTIVE", None)
        else:
            os.environ["HERMES_INTERACTIVE"] = original_interactive

    return {
        "name": "terminal_safety_helpers",
        "compound_background": [
            {"command": command, "rewritten": _rewrite_compound_background(command)}
            for command in rewrite_inputs
        ],
        "foreground_guidance": [
            {"command": command, "guidance": _foreground_background_guidance(command)}
            for command in guidance_inputs
        ],
        "notification_conflicts": [
            {
                **case,
                "resolved_watch_patterns": _resolve_notification_flag_conflict(**case)[0],
                "note": _resolve_notification_flag_conflict(**case)[1],
            }
            for case in notification_inputs
        ],
        "sudo_transform": sudo_cases,
    }


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        cases = [
            {
                "name": "local_defaults",
                "config": with_env({}),
            },
            {
                "name": "docker_sandbox_defaults",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "docker",
                        "TERMINAL_CWD": "/home/user/project",
                    }
                ),
            },
            {
                "name": "docker_mount_cwd",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "docker",
                        "TERMINAL_CWD": "/home/user/project",
                        "TERMINAL_DOCKER_MOUNT_CWD_TO_WORKSPACE": "true",
                        "TERMINAL_DOCKER_FORWARD_ENV": '["SSH_AUTH_SOCK"]',
                        "TERMINAL_DOCKER_ENV": '{"CI": "1"}',
                        "TERMINAL_DOCKER_VOLUMES": '["/tmp:/output"]',
                        "TERMINAL_DOCKER_EXTRA_ARGS": '["--network=none"]',
                    }
                ),
            },
            {
                "name": "ssh_config",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "ssh",
                        "TERMINAL_SSH_HOST": "ssh.example.invalid",
                        "TERMINAL_SSH_USER": "hermes",
                        "TERMINAL_SSH_PORT": "2222",
                        "TERMINAL_SSH_KEY": "/tmp/fake-key",
                        "TERMINAL_TIMEOUT": "45",
                    }
                ),
            },
            {
                "name": "modal_config",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "modal",
                        "TERMINAL_CWD": "/home/user/project",
                        "TERMINAL_MODAL_MODE": "direct",
                        "TERMINAL_CONTAINER_CPU": "2.5",
                        "TERMINAL_CONTAINER_MEMORY": "8192",
                        "TERMINAL_CONTAINER_DISK": "102400",
                        "TERMINAL_CONTAINER_PERSISTENT": "false",
                    }
                ),
            },
            {
                "name": "daytona_config",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "daytona",
                        "TERMINAL_CWD": "/home/user/project",
                    }
                ),
            },
            {
                "name": "singularity_config",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "singularity",
                        "TERMINAL_CWD": "/home/user/project",
                    }
                ),
            },
            {
                "name": "vercel_sandbox_config",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "vercel_sandbox",
                        "TERMINAL_CWD": "/home/user/project",
                    }
                ),
            },
            {
                "name": "persistent_and_modal_coercion",
                "config": with_env(
                    {
                        "TERMINAL_ENV": "ssh",
                        "TERMINAL_LOCAL_PERSISTENT": "yes",
                        "TERMINAL_PERSISTENT_SHELL": "false",
                        "TERMINAL_SSH_PERSISTENT": "true",
                        "TERMINAL_MODAL_MODE": "invalid-mode",
                    }
                ),
            },
            {
                "name": "invalid_timeout_error",
                "result": error_with_env({"TERMINAL_TIMEOUT": "5m"}),
            },
            {
                "name": "invalid_docker_env_json_error",
                "result": error_with_env({"TERMINAL_DOCKER_ENV": "{bad"}),
            },
            {
                "name": "invalid_container_cpu_error",
                "result": error_with_env({"TERMINAL_CONTAINER_CPU": "large"}),
            },
            remote_backend_contracts(home),
            safety_helper_contracts(),
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
