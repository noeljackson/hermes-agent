from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "tool-registry-fixture.py"


def schema_summary(schema):
    parameters = schema.get("parameters") or {}
    properties = parameters.get("properties") or {}
    return {
        "description_present": bool(schema.get("description")),
        "parameter_names": sorted(properties.keys()),
        "required": list(parameters.get("required") or []),
    }


def strip_descriptions(value):
    if isinstance(value, dict):
        return {
            key: strip_descriptions(item)
            for key, item in value.items()
            if key != "description"
        }
    if isinstance(value, list):
        return [strip_descriptions(item) for item in value]
    return value


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from tools.registry import discover_builtin_tools, registry

        selected_schema_names = {
            "browser_navigate",
            "clarify",
            "image_generate",
            "memory",
            "patch",
            "read_file",
            "search_files",
            "session_search",
            "skill_manage",
            "skill_view",
            "skills_list",
            "terminal",
            "text_to_speech",
            "todo",
            "web_extract",
            "web_search",
            "write_file",
        }
        imported_modules = discover_builtin_tools()
        entries = []
        selected_schemas = {}
        for entry in registry._snapshot_entries():
            entries.append(
                {
                    "name": entry.name,
                    "toolset": entry.toolset,
                    "requires_env": list(entry.requires_env),
                    "is_async": entry.is_async,
                    "schema": schema_summary(entry.schema),
                }
            )
            if entry.name in selected_schema_names:
                selected_schemas[entry.name] = strip_descriptions(entry.schema)
        entries.sort(key=lambda item: item["name"])
        cases = [
            {
                "name": "builtin_registry",
                "imported_module_count": len(imported_modules),
                "tool_count": len(entries),
                "toolsets": sorted({entry["toolset"] for entry in entries}),
                "tools": entries,
            },
            {
                "name": "selected_core_schemas",
                "schemas": selected_schemas,
            }
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
