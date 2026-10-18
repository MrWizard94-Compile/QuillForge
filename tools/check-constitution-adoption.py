#!/usr/bin/env python3
"""Classify exact reviewed references after running the pinned canonical audit."""
from __future__ import annotations

import argparse
import hashlib
import stat
import subprocess
import sys
import types
from pathlib import Path

SOURCE_COMMIT = "89f8771df5a6fa8627cb9576afca1eefd0486d27"
SOURCE_SHA256 = "8abf494f014cf6f2595e9b4b281a06d01a394ebebb8fa5f6ff2aefd5c95323ef"

# Only passive historical changelog entries are retained. A whole-file digest
# and exact line set prevent broad
# exemptions from hiding new consumers. CRLF normalization permits Git checkout
# conventions; no other bytes or finding codes are ignored.
REVIEWED = {
    "CHANGELOG.md": ("504e6b2cdd685376285daffc8a1d27195d28607029eda5075c1d3e4a7010252f", {53, 73}),
}


def digest(content: bytes) -> str:
    return hashlib.sha256(content.replace(b"\r\n", b"\n")).hexdigest()


def load_checker(source: Path) -> types.ModuleType:
    content = source.read_bytes()
    if digest(content) != SOURCE_SHA256:
        raise ValueError(f"canonical checker differs from pinned source {SOURCE_COMMIT}")
    checker = types.ModuleType("wpai_canonical_adoption")
    checker.__file__ = str(source)
    # Execute the same bytes that were verified, not a second mutable file read.
    exec(compile(content, str(source), "exec"), checker.__dict__)
    return checker


def classify(
    project: Path,
    findings: list[tuple[str, str, int]],
    reviewed: dict[str, tuple[str, set[int]]] = REVIEWED,
) -> tuple[list[tuple[str, str, int]], list[str]]:
    retained: list[tuple[str, str, int]] = []
    problems: list[str] = []
    expected_findings = {
        ("LEGACY_REFERENCE", relative, line)
        for relative, (_, lines) in reviewed.items() for line in lines
    }
    valid_paths: set[str] = set()
    for relative, (expected_hash, _) in reviewed.items():
        candidate = project / relative
        try:
            if not candidate.resolve(strict=True).is_relative_to(project.resolve(strict=True)):
                raise ValueError("reviewed file escapes project")
            for current in (candidate, *candidate.parents):
                metadata = current.lstat()
                if stat.S_ISLNK(metadata.st_mode) or (
                    getattr(metadata, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
                ):
                    raise ValueError("reviewed path is linked")
                if current == project:
                    break
            if digest(candidate.read_bytes()) != expected_hash:
                raise ValueError("reviewed file changed; re-review required")
            valid_paths.add(relative)
        except (OSError, ValueError) as error:
            problems.append(f"{relative}: {error}")
    seen: set[tuple[str, str, int]] = set()
    for finding in findings:
        if finding in seen:
            problems.append(f"duplicate finding: {finding}")
        seen.add(finding)
        if finding in expected_findings and finding[1] in valid_paths:
            retained.append(finding)
        else:
            problems.append(f"unreviewed finding: {finding}")
    for missing in sorted(expected_findings - seen):
        problems.append(f"expected reviewed evidence missing: {missing}")
    return retained, problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="accepted canonical adoption checker")
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    try:
        checker = load_checker(args.source.resolve(strict=True))
        retained, problems = classify(project, checker.inspect(project))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"FAIL: audit could not complete: {error}", file=sys.stderr)
        return 2
    for _, relative, line in retained:
        print(f"RETAINED_NONACTIVE: {relative}:{line} (exact file hash and line verified)")
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"PASS: pinned canonical audit; {len(retained)} explicitly reviewed non-active references retained")
    print("Scope: Git project and bounded agent-config paths; excluded caches and live consumers are separate gates.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
