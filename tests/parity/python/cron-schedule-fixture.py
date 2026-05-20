from __future__ import annotations

from datetime import datetime, timezone

from parity_common import (
    fixture,
    isolated_hermes_home,
    normalize_timestamps,
    parse_out_arg,
    write_fixture,
)


SCRIPT = "cron-schedule-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from cron.jobs import (
            _compute_grace_seconds,
            _normalize_job_record,
            compute_next_run,
            load_jobs,
            parse_schedule,
            save_jobs,
        )
        import cron.jobs as cron_jobs

        fixed_now = datetime(2026, 5, 20, 12, 0, 0, tzinfo=timezone.utc)
        cron_jobs._hermes_now = lambda: fixed_now

        parsed = {
            "interval": parse_schedule("every 2h"),
            "cron": parse_schedule("0 9 * * *"),
            "timestamp": parse_schedule("2026-06-01T09:00:00+00:00"),
        }
        legacy_record = _normalize_job_record(
            {
                "id": "job-1",
                "prompt": None,
                "schedule": parsed["cron"],
                "enabled": True,
                "skills": ["demo"],
                "created_at": "2026-01-01T00:00:00+00:00",
            }
        )
        normalization_matrix = {
            "single_skill_string": _normalize_job_record(
                {
                    "id": "job-skill-string",
                    "prompt": "Prompt",
                    "skill": "demo",
                    "schedule": {"display": "every 5m"},
                    "enabled": True,
                }
            ),
            "skills_string_overrides_legacy": _normalize_job_record(
                {
                    "id": "job-skills-string",
                    "prompt": "Prompt",
                    "skill": "legacy",
                    "skills": "demo",
                    "schedule": {"display": "every 5m"},
                    "enabled": True,
                }
            ),
            "skills_list_dedupes": _normalize_job_record(
                {
                    "id": "job-skills-dedupe",
                    "prompt": "Prompt",
                    "skills": ["demo", "", None, "demo", "other"],
                    "schedule": {"display": "every 5m"},
                    "enabled": True,
                }
            ),
            "schedule_display_wins": _normalize_job_record(
                {
                    "id": "job-display",
                    "prompt": "Prompt",
                    "schedule_display": "custom display",
                    "schedule": {"display": "ignored", "expr": "0 9 * * *"},
                    "enabled": True,
                }
            ),
            "schedule_value_fallback": _normalize_job_record(
                {
                    "id": "job-value",
                    "prompt": "Prompt",
                    "schedule": {"value": "value display"},
                    "enabled": True,
                }
            ),
            "schedule_expr_fallback": _normalize_job_record(
                {
                    "id": "job-expr",
                    "prompt": "Prompt",
                    "schedule": {"expr": "0 9 * * *"},
                    "enabled": True,
                }
            ),
            "schedule_run_at_fallback": _normalize_job_record(
                {
                    "id": "job-run-at",
                    "prompt": "Prompt",
                    "schedule": {"run_at": "2026-06-01T09:00:00+00:00"},
                    "enabled": True,
                }
            ),
            "schedule_string_fallback": _normalize_job_record(
                {
                    "id": "job-schedule-string",
                    "prompt": "Prompt",
                    "schedule": "every 10m",
                    "enabled": True,
                }
            ),
            "name_from_script": _normalize_job_record(
                {
                    "id": "job-script",
                    "prompt": None,
                    "script": "/tmp/demo.sh",
                    "schedule": {"display": "manual"},
                    "enabled": True,
                }
            ),
            "name_from_id_paused_profile": _normalize_job_record(
                {
                    "id": "job-id",
                    "prompt": None,
                    "schedule": {},
                    "enabled": False,
                    "profile": " ",
                }
            ),
        }
        interval_30 = {"kind": "interval", "minutes": 30, "display": "every 30m"}
        once_future = {
            "kind": "once",
            "run_at": "2026-05-20T12:05:00+00:00",
            "display": "once at 2026-05-20 12:05",
        }
        once_grace = {
            "kind": "once",
            "run_at": "2026-05-20T11:59:00+00:00",
            "display": "once at 2026-05-20 11:59",
        }
        once_expired = {
            "kind": "once",
            "run_at": "2026-05-20T11:00:00+00:00",
            "display": "once at 2026-05-20 11:00",
        }
        scheduler_cases = {
            "interval_first_run": compute_next_run(interval_30),
            "interval_after_last_run": compute_next_run(
                interval_30,
                last_run_at="2026-05-20T11:00:00+00:00",
            ),
            "once_future": compute_next_run(once_future),
            "once_within_grace": compute_next_run(once_grace),
            "once_expired": compute_next_run(once_expired),
            "once_already_run": compute_next_run(
                once_future,
                last_run_at="2026-05-20T12:01:00+00:00",
            ),
            "grace_1m": _compute_grace_seconds({"kind": "interval", "minutes": 1}),
            "grace_10m": _compute_grace_seconds({"kind": "interval", "minutes": 10}),
            "grace_1d": _compute_grace_seconds({"kind": "interval", "minutes": 1440}),
        }
        save_jobs([legacy_record])
        loaded = load_jobs()
        cases = [
            {"name": "parse_schedule", "schedules": parsed},
            {
                "name": "legacy_record_normalization",
                "job": normalize_timestamps(legacy_record),
            },
            {
                "name": "record_normalization_matrix",
                "jobs": normalize_timestamps(normalization_matrix),
            },
            {
                "name": "scheduler_time_math",
                "fixed_now": fixed_now.isoformat(),
                "results": normalize_timestamps(scheduler_cases),
            },
            {
                "name": "storage_shape",
                "jobs": normalize_timestamps(loaded),
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
