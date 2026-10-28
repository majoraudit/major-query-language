"""Installed-wheel smoke tests for the public Python API."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import pylibmql


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "docs" / "fixtures" / "semantic-ir" / "reference-fixtures.json"


class PythonApiTests(unittest.TestCase):
    def test_package_version(self) -> None:
        self.assertEqual(pylibmql.__version__, "0.2.0")

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


if __name__ == "__main__":
    unittest.main()
