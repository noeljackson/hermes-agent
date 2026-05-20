from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "file-tools-fixture.py"


class FakeEnv:
    cwd = "/tmp"

    def execute(self, command, cwd=None, **kwargs):
        return {"output": "", "returncode": 0}


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from tools.file_operations import (
            ShellFileOperations,
            _is_write_denied,
            normalize_read_pagination,
            normalize_search_pagination,
        )
        from tools.fuzzy_match import fuzzy_find_and_replace

        ops = ShellFileOperations(FakeEnv())
        content = "line one\nline two\n" + ("x" * 2105)
        binary_content = "\x00\x01\x02\x03" * 250
        text_content = "Hello world\nLine 2\n"
        replace_cases = {}
        for name, args in {
            "exact": ("alpha beta alpha", "beta", "BETA", False),
            "multiple_error": ("same same", "same", "other", False),
            "replace_all": ("same same", "same", "other", True),
            "empty_old": ("abc", "", "x", False),
            "identical": ("abc", "abc", "abc", False),
            "not_found": ("abc", "missing", "x", False),
            "unicode_normalized": ("hello -- world", "hello — world", "hi — world", False),
        }.items():
            new_content, count, strategy, error = fuzzy_find_and_replace(*args)
            replace_cases[name] = {
                "content": new_content,
                "count": count,
                "strategy": strategy,
                "error": error,
            }

        cases = [
            {
                "name": "pagination",
                "read": {
                    "zero": normalize_read_pagination(0, 0),
                    "negative": normalize_read_pagination(-10, -5),
                    "bad": normalize_read_pagination("bad", "bad"),
                    "max": normalize_read_pagination(2, 999999),
                },
                "search": {
                    "negative": normalize_search_pagination(-10, -5),
                    "bad": normalize_search_pagination("bad", "bad"),
                    "zero_limit": normalize_search_pagination(3, 0),
                },
            },
            {
                "name": "write_deny",
                "paths": {
                    "ssh_authorized_keys": _is_write_denied("~/.ssh/authorized_keys"),
                    "ssh_private_key": _is_write_denied("~/.ssh/id_rsa"),
                    "netrc": _is_write_denied("~/.netrc"),
                    "aws_credentials": _is_write_denied("~/.aws/credentials"),
                    "kube_config": _is_write_denied("~/.kube/config"),
                    "project_file": _is_write_denied("/tmp/project/main.py"),
                },
            },
            {
                "name": "classification",
                "binary": {
                    "png": ops._is_likely_binary("photo.png"),
                    "sqlite": ops._is_likely_binary("data.db"),
                    "python": ops._is_likely_binary("code.py"),
                    "binary_content": ops._is_likely_binary("unknown", binary_content),
                    "text_content": ops._is_likely_binary("unknown", text_content),
                },
                "image": {
                    "png": ops._is_image("photo.png"),
                    "jpg": ops._is_image("pic.jpg"),
                    "ico": ops._is_image("icon.ico"),
                    "pdf": ops._is_image("data.pdf"),
                    "py": ops._is_image("code.py"),
                },
            },
            {
                "name": "line_numbers",
                "default": ops._add_line_numbers("line one\nline two\nline three"),
                "offset": ops._add_line_numbers("continued\nmore", 50),
                "truncated": ops._add_line_numbers(content),
            },
            {
                "name": "fuzzy_replace",
                "cases": replace_cases,
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
