#!/usr/bin/env python3
"""Validate the Mendrel design pack without invoking the Mendrel compiler.

Base checks use only the Python standard library. If `jsonschema` is installed,
JSON Schema meta-validation and example validation are also performed.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import sys
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]

PACK_ROOT_FILES = {
    "AGENTS.md",
    "PROMPT_FOR_CODEX.md",
    "README.md",
    "VALIDATION.md",
}
PACK_DIRECTORIES = ("docs", "examples", "schemas", "scripts", "spec")


def fail(message: str) -> None:
    print(f"FAIL: {message}", file=sys.stderr)
    raise SystemExit(1)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest_rows() -> list[tuple[str, int, str]]:
    text = (ROOT / "MANIFEST.md").read_text(encoding="utf-8")
    rows = re.findall(
        r"^\| `([^`]+)` \| ([0-9,]+) \| `([0-9a-f]{64})` \|",
        text,
        flags=re.MULTILINE,
    )
    return [(path, int(size.replace(",", "")), digest) for path, size, digest in rows]


def check_manifest() -> None:
    rows = manifest_rows()
    expected_paths = set(PACK_ROOT_FILES)
    for directory in PACK_DIRECTORIES:
        for path in (ROOT / directory).rglob("*"):
            relative = path.relative_to(ROOT)
            if (
                path.is_file()
                and "__pycache__" not in path.parts
                and "superpowers" not in relative.parts
            ):
                expected_paths.add(str(relative))
    row_paths = {path for path, _, _ in rows}
    if row_paths != expected_paths:
        missing = sorted(expected_paths - row_paths)
        extra = sorted(row_paths - expected_paths)
        fail(f"manifest path mismatch; missing={missing}, extra={extra}")

    for rel, expected_size, expected_digest in rows:
        path = ROOT / rel
        actual_size = path.stat().st_size
        actual_digest = sha256(path)
        if actual_size != expected_size:
            fail(f"{rel}: expected {expected_size} bytes, found {actual_size}")
        if actual_digest != expected_digest:
            fail(f"{rel}: digest mismatch")
    print(f"OK: manifest covers {len(rows)} files and all digests match")


def strip_ebnf_comments(text: str) -> str:
    return re.sub(r"\(\*.*?\*\)", "", text, flags=re.DOTALL)


def check_ebnf() -> None:
    text = strip_ebnf_comments((ROOT / "spec/grammar.ebnf").read_text(encoding="utf-8"))
    productions = {
        match.group(1): match.group(2)
        for match in re.finditer(
            r"(?ms)^\s*([a-z_][a-z0-9_]*)\s*=\s*(.*?);",
            text,
        )
    }
    if not productions:
        fail("no EBNF productions found")

    references: set[str] = set()
    for rhs in productions.values():
        without_literals = re.sub(r'"(?:[^"\\]|\\.)*"', " ", rhs)
        references.update(re.findall(r"\b[a-z_][a-z0-9_]*\b", without_literals))

    undefined = sorted(references - productions.keys())
    roots = {"source_file", "manifest_file"}
    unreferenced = sorted(productions.keys() - references - roots)
    if undefined:
        fail(f"undefined EBNF nonterminals: {undefined}")
    if unreferenced:
        fail(f"unreferenced EBNF productions: {unreferenced}")
    print(f"OK: {len(productions)} EBNF productions; no undefined or orphan production")


def check_json_parse() -> None:
    json_files = sorted((ROOT / "schemas").glob("*.json")) + [
        ROOT / "examples/map-request.json"
    ]
    for path in json_files:
        json.loads(path.read_text(encoding="utf-8"))
    for line_no, line in enumerate(
        (ROOT / "examples/diagnostic.jsonl").read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        if line.strip():
            try:
                json.loads(line)
            except json.JSONDecodeError as error:
                fail(f"examples/diagnostic.jsonl:{line_no}: {error}")
    print(f"OK: parsed {len(json_files)} JSON files and diagnostic JSONL")


def check_json_schema(strict: bool) -> None:
    try:
        import jsonschema  # type: ignore
    except ImportError:
        if strict:
            fail("jsonschema is required by --strict-schema")
        print("SKIP: jsonschema package unavailable; structural JSON parsing still passed")
        return

    diagnostic_schema = json.loads(
        (ROOT / "schemas/diagnostic-v1.schema.json").read_text(encoding="utf-8")
    )
    map_schema = json.loads((ROOT / "schemas/map-v1.schema.json").read_text(encoding="utf-8"))
    jsonschema.Draft202012Validator.check_schema(diagnostic_schema)
    jsonschema.Draft202012Validator.check_schema(map_schema)

    diagnostic_validator = jsonschema.Draft202012Validator(diagnostic_schema)
    for line_no, line in enumerate(
        (ROOT / "examples/diagnostic.jsonl").read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        if not line.strip():
            continue
        errors = sorted(
            diagnostic_validator.iter_errors(json.loads(line)),
            key=lambda error: list(error.absolute_path),
        )
        if errors:
            fail(f"diagnostic example line {line_no}: {errors[0].message}")

    map_validator = jsonschema.Draft202012Validator(map_schema)
    map_errors = sorted(
        map_validator.iter_errors(
            json.loads((ROOT / "examples/map-request.json").read_text(encoding="utf-8"))
        ),
        key=lambda error: list(error.absolute_path),
    )
    if map_errors:
        fail(f"MAP example: {map_errors[0].message}")
    print("OK: both schemas meta-validate and both example families conform")


def check_markdown() -> None:
    unbalanced: list[str] = []
    placeholders: list[str] = []
    placeholder_pattern = re.compile(r"\b(?:TODO|TBD|FIXME|XXX)\b")
    for path in sorted(ROOT.rglob("*.md")):
        text = path.read_text(encoding="utf-8")
        fence_count = sum(1 for line in text.splitlines() if line.startswith("```"))
        if fence_count % 2:
            unbalanced.append(str(path.relative_to(ROOT)))
        for line_no, line in enumerate(text.splitlines(), start=1):
            if placeholder_pattern.search(line):
                # The roadmap intentionally rejects placeholders; that sentence is not one.
                if "phase 外の placeholder" not in line:
                    placeholders.append(f"{path.relative_to(ROOT)}:{line_no}")
    if unbalanced:
        fail(f"unbalanced Markdown code fences: {unbalanced}")
    if placeholders:
        fail(f"placeholder markers found: {placeholders}")
    print("OK: Markdown fences are balanced and no unresolved placeholder marker exists")


def check_examples() -> None:
    source = (ROOT / "examples/checkout_service.mnd").read_text(encoding="utf-8")
    bindings = re.findall(r"\blet\s+([A-Za-z_][A-Za-z0-9_]*)\b", source)
    duplicates = sorted(name for name, count in collections.Counter(bindings).items() if count > 1)
    if duplicates:
        fail(f"example violates no-shadowing intent: {duplicates}")
    if re.search(r"\.[A-Za-z_][A-Za-z0-9_]*\(move\s+[A-Za-z_]", source):
        fail("example passes an explicit receiver to a receiver-consuming method")
    if 'module: "shop.checkout";' in (ROOT / "examples/Mendrel.pkg").read_text(encoding="utf-8"):
        fail("manifest example uses statement punctuation inside data object")
    print("OK: production examples satisfy checked pack-level syntax invariants")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--strict-schema",
        action="store_true",
        help="fail when the optional jsonschema package is unavailable",
    )
    args = parser.parse_args()

    check_manifest()
    check_ebnf()
    check_json_parse()
    check_json_schema(args.strict_schema)
    check_markdown()
    check_examples()
    print("PASS: Mendrel design pack validation completed")


if __name__ == "__main__":
    main()
