#!/usr/bin/env python3
"""Check/rewrite legacy graph version selectors to strict semver.

This is intentionally conservative and only touches schema-known YAML keys:
- `version: ...`
- `impl: id@selector`
- `cluster: id@selector`
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


SEMVER_EXACT_RE = re.compile(
    r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
)
VERSION_LINE_RE = re.compile(
    r"^(?P<indent>\s*)version:\s*(?P<value>[^#\n]+?)(?P<comment>\s*(?:#.*)?)$"
)
PACKED_LINE_RE = re.compile(
    r"^(?P<indent>\s*)(?P<field>impl|cluster):\s*(?P<value>[^#\n]+?)(?P<comment>\s*(?:#.*)?)$"
)


@dataclass
class Finding:
    path: Path
    line_no: int
    message: str
    rewrite: str | None = None


def strip_quotes(value: str) -> tuple[str, str]:
    raw = value.strip()
    if len(raw) >= 2 and raw[0] == raw[-1] and raw[0] in {"'", '"'}:
        return raw[1:-1], raw[0]
    return raw, ""


def quote(value: str, quote_char: str) -> str:
    if quote_char:
        return f"{quote_char}{value}{quote_char}"
    return value


def normalize_v_prefixed(tag: str) -> str | None:
    if not tag.startswith("v"):
        return None
    rest = tag[1:]
    if SEMVER_EXACT_RE.fullmatch(rest):
        return rest
    if re.fullmatch(r"\d+", rest):
        return f"{rest}.0.0"
    if re.fullmatch(r"\d+\.\d+", rest):
        major, minor = rest.split(".")
        return f"{major}.{minor}.0"
    return None


def is_strict_semver(value: str) -> bool:
    return bool(SEMVER_EXACT_RE.fullmatch(value))


def analyze_file(path: Path, rewrite: bool) -> tuple[list[Finding], str | None]:
    original = path.read_text()
    lines = original.splitlines()
    findings: list[Finding] = []
    changed = False

    for idx, line in enumerate(lines):
        line_no = idx + 1
        m = VERSION_LINE_RE.match(line)
        if m:
            value_raw, quote_char = strip_quotes(m.group("value"))
            replacement_value = None
            if is_strict_semver(value_raw):
                continue
            if (rw := normalize_v_prefixed(value_raw)) is not None:
                replacement_value = rw
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"legacy version '{value_raw}' can be rewritten to '{rw}'",
                        rewrite=f"{m.group('indent')}version: {quote(rw, quote_char)}{m.group('comment')}",
                    )
                )
            else:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"manual migration required for version '{value_raw}' (expected strict semver)",
                    )
                )
            if rewrite and replacement_value is not None:
                lines[idx] = f"{m.group('indent')}version: {quote(replacement_value, quote_char)}{m.group('comment')}"
                changed = True
            continue

        m = PACKED_LINE_RE.match(line)
        if not m:
            continue

        packed_raw, quote_char = strip_quotes(m.group("value"))
        if "@" not in packed_raw:
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"manual migration required for {m.group('field')} reference '{packed_raw}' (expected '<id>@<version>')",
                )
            )
            continue
        ref_id, selector = packed_raw.rsplit("@", 1)
        if not selector:
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"manual migration required for {m.group('field')} reference '{packed_raw}' (empty version selector)",
                )
            )
            continue

        if is_strict_semver(selector):
            continue

        replacement_selector = normalize_v_prefixed(selector)
        if replacement_selector is None:
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"manual migration required for {m.group('field')} selector '{selector}'",
                )
            )
            continue

        rewritten = f"{ref_id}@{replacement_selector}"
        findings.append(
            Finding(
                path,
                line_no,
                f"legacy {m.group('field')} selector '{selector}' can be rewritten to '{replacement_selector}'",
                rewrite=f"{m.group('indent')}{m.group('field')}: {quote(rewritten, quote_char)}{m.group('comment')}",
            )
        )
        if rewrite:
            lines[idx] = f"{m.group('indent')}{m.group('field')}: {quote(rewritten, quote_char)}{m.group('comment')}"
            changed = True

    if rewrite and changed:
        return findings, "\n".join(lines) + ("\n" if original.endswith("\n") else "")
    return findings, None


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="report legacy versions")
    mode.add_argument(
        "--rewrite-v-prefix",
        action="store_true",
        help="rewrite deterministic v-prefixed forms only",
    )
    parser.add_argument("paths", nargs="+", help="YAML files to inspect")
    args = parser.parse_args()

    any_manual = False
    any_findings = False

    for raw_path in args.paths:
        path = Path(raw_path)
        if not path.exists():
            print(f"[missing] {path}", file=sys.stderr)
            any_manual = True
            continue
        findings, rewritten = analyze_file(path, rewrite=args.rewrite_v_prefix)
        if rewritten is not None:
            path.write_text(rewritten)
        for finding in findings:
            any_findings = True
            print(f"{finding.path}:{finding.line_no}: {finding.message}")
            if "manual migration required" in finding.message:
                any_manual = True

    if not any_findings:
        print("No legacy version selectors found.")
    return 1 if any_manual else 0


if __name__ == "__main__":
    raise SystemExit(main())
