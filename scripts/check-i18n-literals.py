#!/usr/bin/env python3
"""Find user-facing Rust string literals that bypass the i18n catalog.

The scanner uses rust-analyzer's syntax-tree output when the binary is
available and falls back to a balanced-token lexer. It intentionally focuses
on UI/toast/event sinks; protocol markers, game-log parsing, identifiers,
paths, tracing-only diagnostics, and test fixtures are outside its scope.
"""

from __future__ import annotations

import ast
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_FILES = [
    ROOT / "src/main.rs",
    ROOT / "src/audio.rs",
    ROOT / "src/keyboard.rs",
    ROOT / "src/log_reader.rs",
    ROOT / "src/osc.rs",
    ROOT / "src/runtime.rs",
]

SINKS = {
    "Typography::new",
    "Typography::small",
    "Typography::muted",
    "Typography::h3",
    "Badge::new",
    "PropertyRow::new",
    "ShadcnLabel::new",
    "ShadcnButton::new",
    "RichText::new",
    "page_heading",
    "section_card",
    ".label",
    ".placeholder",
    ".suffix",
    ".on_hover_text",
    ".title",
    ".description",
    ".cancel_text",
    ".action_text",
    ".x_axis_label",
    ".y_axis_label",
    ".add",
    ".event",
}

# Brand/protocol notation and purely symbolic glyphs are intentionally stable.
ALLOW = {
    "ECLIPTICA",
    "DATA ANALYZER",
    "DPS",
    "●",
    "-",
    "127.0.0.1:9000",
}

STRING = re.compile(r'(?s)(?:b|c)?r(?P<hash>#{0,255})".*?"(?P=hash)|"(?:\\.|[^"\\])*"')


def mask_tests(source: str) -> str:
    marker = source.find("#[cfg(test)]")
    return source if marker < 0 else source[:marker]


def decode(literal: str) -> str:
    prefix = re.match(r'(?:b|c)?r(#{0,255})"', literal)
    if prefix:
        hashes = prefix.group(1)
        return literal[prefix.end() : -(len(hashes) + 1)]
    if literal.startswith(('b"', 'c"')):
        literal = literal[1:]
    try:
        return ast.literal_eval(literal)
    except (SyntaxError, ValueError):
        return literal


def sink_before(source: str, start: int) -> str | None:
    # A bounded prefix is enough for direct arguments while avoiding unrelated
    # calls earlier in the function. Balanced delimiters permit multiline calls.
    prefix = source[max(0, start - 500) : start]
    best: tuple[int, str] | None = None
    for sink in SINKS:
        match = list(re.finditer(re.escape(sink) + r"\s*\(", prefix))
        if match and (best is None or match[-1].start() > best[0]):
            best = (match[-1].start(), sink)
    if best is None:
        return None
    tail = prefix[best[0] :]
    depth = 0
    in_string = False
    escaped = False
    for char in tail:
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue
        if char == '"':
            in_string = True
        elif char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
    return best[1] if depth > 0 else None


def ast_literal_spans(path: Path) -> list[tuple[int, int]] | None:
    try:
        result = subprocess.run(
            ["rust-analyzer", "syntax-tree", str(path)],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=5,
            check=False,
        )
        if result.returncode != 0:
            return None
        spans = [
            (int(start), int(end))
            for start, end in re.findall(
                r"(?:STRING|BYTE_STRING|C_STRING|RAW_STRING|RAW_BYTE_STRING)@(\d+)\.\.(\d+)",
                result.stdout,
            )
        ]
        return spans or None
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None


def main() -> int:
    findings: list[str] = []
    used_ast = False
    for path in SOURCE_FILES:
        source = mask_tests(path.read_text(encoding="utf-8"))
        spans = ast_literal_spans(path)
        used_ast |= spans is not None
        matches = (
            [match for match in STRING.finditer(source) if (match.start(), match.end()) in spans]
            if spans is not None
            else STRING.finditer(source)
        )
        for match in matches:
            value = decode(match.group(0))
            semantic = re.sub(r"\{[^{}]*\}", "", value)
            if (
                value in ALLOW
                or not any(char.isalpha() for char in semantic)
                or re.fullmatch(r"[A-Za-z0-9_.:/-]+", value)
                or re.fullmatch(r"v\{[A-Z_]+\}", value)
                or re.fullmatch(r"ECLIPTICA\s+v\{[A-Z_]+\}", value)
            ):
                continue
            sink = sink_before(source, match.start())
            if sink is None:
                continue
            line = source.count("\n", 0, match.start()) + 1
            findings.append(f"{path.relative_to(ROOT)}:{line}: {sink}: {value!r}")

    backend = "rust-analyzer AST" if used_ast else "balanced-token fallback"
    if findings:
        print(f"i18n literal check ({backend}) found {len(findings)} candidate(s):")
        print("\n".join(findings))
        return 1
    print(f"i18n literal check passed ({backend})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
