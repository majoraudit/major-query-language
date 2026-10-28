#!/usr/bin/env python3
"""Ensure every published component shares one version and the IR matches its schema."""

from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def cargo_version(path: str) -> str:
    with (ROOT / path).open("rb") as source:
        return tomllib.load(source)["package"]["version"]


def main() -> None:
    versions = {
        "pylibmql": cargo_version("Cargo.toml"),
        "libmql": cargo_version("libmql/Cargo.toml"),
        "mql-cli": cargo_version("cli/Cargo.toml"),
    }
    if len(set(versions.values())) != 1:
        raise SystemExit(f"release versions disagree: {versions}")

    version = next(iter(versions.values()))

    # The IR wire version moves independently of the package (RFC 0001 §2),
    # but the Rust constant and the normative schema must agree.
    semantic_source = (ROOT / "libmql/src/semantic_ir.rs").read_text(encoding="utf-8")
    match = re.search(r'SEMANTIC_IR_VERSION:\s*&str\s*=\s*"([^"]+)"', semantic_source)
    schema = json.loads((ROOT / "docs/schemas/mql-ir-0.2.schema.json").read_text(encoding="utf-8"))
    schema_version = schema["properties"]["schema_version"]["const"]
    if match is None or match.group(1) != schema_version:
        found = match.group(1) if match else None
        raise SystemExit(f"SEMANTIC_IR_VERSION {found} does not match schema {schema_version}")

    expected_tag = f"v{version}"
    if len(sys.argv) > 1 and sys.argv[1] != expected_tag:
        raise SystemExit(f"release tag must be {expected_tag}, found {sys.argv[1]}")

    print(f"release versions agree at {version}; expected tag {expected_tag}")


if __name__ == "__main__":
    main()
