#!/usr/bin/env python3
"""Validate the RFC schemas, positive fixtures, and schema-negative cases.

With `--compiler PATH` (the `mql` binary), also compile every MQL 0.2 fixture
source and validate the emitted IR against the normative schema.
"""

from __future__ import annotations

import argparse
import copy
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
SCHEMAS = DOCS / "schemas"
FIXTURES = DOCS / "fixtures" / "semantic-ir"


def load(path: Path) -> Any:
    with path.open(encoding="utf-8") as source:
        return json.load(source)


def resolve_pointer(document: Any, reference: str) -> Any:
    _, pointer = reference.split("#/", 1)
    current = document
    for raw_part in pointer.split("/"):
        part = raw_part.replace("~1", "/").replace("~0", "~")
        current = current[int(part)] if isinstance(current, list) else current[part]
    return current


def apply_operation(document: Any, operation: dict[str, Any]) -> None:
    parts = operation["path"].strip("/").split("/")
    parent = document
    for part in parts[:-1]:
        parent = parent[int(part)] if isinstance(parent, list) else parent[part]
    key = parts[-1]
    if operation["op"] == "remove":
        if isinstance(parent, list):
            del parent[int(key)]
        else:
            del parent[key]
    elif operation["op"] in {"add", "replace"}:
        if isinstance(parent, list):
            parent[int(key)] = operation["value"]
        else:
            parent[key] = operation["value"]
    else:
        raise AssertionError(f"unsupported fixture operation: {operation['op']}")


def validate(schema: dict[str, Any], value: Any, label: str) -> None:
    errors = sorted(Draft202012Validator(schema).iter_errors(value), key=lambda error: list(error.path))
    if errors:
        rendered = "\n".join(f"{label} /{'/'.join(map(str, error.path))}: {error.message}" for error in errors)
        raise AssertionError(rendered)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, help="path to the mql binary")
    compiler = parser.parse_args().compiler
    schema_names = [
        "institution-schema",
        "mql-ir",
        "catalog",
        "transcript",
        "profile",
        "audit-evaluation",
        "reference-fixtures",
    ]
    schemas = {name: load(SCHEMAS / f"{name}-0.2.schema.json") for name in schema_names}
    for name, schema in schemas.items():
        Draft202012Validator.check_schema(schema)

    fixture_document = load(FIXTURES / "reference-fixtures.json")
    validate(schemas["institution-schema"], fixture_document["institution"], "institution")
    assert len(fixture_document["fixtures"]) == 8

    for fixture in fixture_document["fixtures"]:
        fixture_id = fixture["fixture_id"]
        validate(schemas["mql-ir"], fixture["program"], f"{fixture_id}:program")
        validate(schemas["catalog"], fixture["catalog"], f"{fixture_id}:catalog")
        for index, case in enumerate(fixture["cases"]):
            validate(schemas["transcript"], case["transcript"], f"{fixture_id}:case{index}:transcript")
            validate(schemas["profile"], case["profile"], f"{fixture_id}:case{index}:profile")
            validate(schemas["audit-evaluation"], case["expected"], f"{fixture_id}:case{index}:expected")

    negative = load(FIXTURES / "negative-cases.json")
    for case in negative["schema_cases"]:
        reference = case.get("base", negative["base"])
        mutated = copy.deepcopy(resolve_pointer(fixture_document, reference))
        apply_operation(mutated, case["operation"])
        errors = list(Draft202012Validator(schemas["mql-ir"]).iter_errors(mutated))
        assert errors, f"{case['case_id']} unexpectedly passed validation"
        assert any(error.validator == case["expected_keyword"] for error in errors), case["case_id"]
        assert any(case["expected_message_contains"] in error.message for error in errors), case["case_id"]

    print("validated 7 schemas, 8 fixture families, and 4 negative schema cases")

    if compiler is not None:
        with tempfile.TemporaryDirectory() as scratch:
            institution = Path(scratch) / "institution.json"
            institution.write_text(json.dumps(fixture_document["institution"]), encoding="utf-8")
            sources = sorted((FIXTURES / "source").glob("*.mql"))
            assert len(sources) == 8, sources
            for source in sources:
                result = subprocess.run(
                    [compiler, "--compile", str(source), "--institution", str(institution)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if result.returncode != 0:
                    raise AssertionError(f"{source.name} failed to compile:\n{result.stderr}")
                validate(schemas["mql-ir"], json.loads(result.stdout), f"compiled {source.name}")
        print(f"compiled {len(sources)} MQL 0.2 sources to schema-valid IR")


if __name__ == "__main__":
    main()
