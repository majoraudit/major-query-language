#!/usr/bin/env python3
"""Ensure every published component and the semantic schema share one version."""

from __future__ import annotations

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
    semantic_source = (ROOT / "libmql/src/semantic_ir.rs").read_text(encoding="utf-8")
    match = re.search(r'SEMANTIC_IR_VERSION:\s*&str\s*=\s*"([^"]+)"', semantic_source)
    if match is None or match.group(1) != version:
        raise SystemExit(f"semantic IR version does not match package {version}")

    expected_tag = f"v{version}"
    if len(sys.argv) > 1 and sys.argv[1] != expected_tag:
        raise SystemExit(f"release tag must be {expected_tag}, found {sys.argv[1]}")

    print(f"release versions agree at {version}; expected tag {expected_tag}")


if __name__ == "__main__":
    main()
