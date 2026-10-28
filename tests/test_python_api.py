"""Installed-wheel smoke tests for the public Python API."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import pylibmql


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "docs" / "fixtures" / "semantic-ir" / "reference-fixtures.json"
SOURCES = ROOT / "docs" / "fixtures" / "semantic-ir" / "source"


class PythonApiTests(unittest.TestCase):
    def test_package_version(self) -> None:
        self.assertEqual(pylibmql.__version__, "0.3.0")

    def test_legacy_parser_keeps_legacy_language_version(self) -> None:
        result = pylibmql.parse('SELECT 1 FROM MATH 2250 : "Required";')
        self.assertEqual(result.version(), "0.1.12")
        self.assertEqual(json.loads(result.json())["version"], "0.1.12")

    def test_semantic_ir_fixture_validates_and_round_trips(self) -> None:
        with FIXTURES.open(encoding="utf-8") as source:
            program = json.load(source)["fixtures"][0]["program"]
        result = pylibmql.validate_semantic_ir(json.dumps(program))
        self.assertEqual(result.version(), "0.2.0")
        self.assertEqual(json.loads(result.json())["program_id"], "cs_systems")

    def test_invalid_semantic_reference_is_rejected(self) -> None:
        with FIXTURES.open(encoding="utf-8") as source:
            program = json.load(source)["fixtures"][0]["program"]
        program["requirements"][0]["constraint_ids"] = ["missing"]
        with self.assertRaisesRegex(ValueError, "MQL0201_UNRESOLVED_ID"):
            pylibmql.validate_semantic_ir(json.dumps(program))

    def test_compiles_mql_source_with_institution_schema(self) -> None:
        with FIXTURES.open(encoding="utf-8") as source:
            institution = json.load(source)["institution"]
        result = pylibmql.compile_program(
            (SOURCES / "ee.mql").read_text(encoding="utf-8"),
            "ee.mql",
            json.dumps(institution),
        )
        program = json.loads(result.json())
        self.assertEqual(result.version(), "0.2.0")
        self.assertEqual(program["program_id"], "ee_electives")
        self.assertEqual(program["constraints"][0]["meta"]["id"], "three_advanced")

    def test_compile_errors_report_code_and_position(self) -> None:
        with self.assertRaisesRegex(ValueError, r"bad\.mql:1:1: MQL0300_SYNTAX"):
            pylibmql.compile_program("requirement x {}", "bad.mql")


if __name__ == "__main__":
    unittest.main()
