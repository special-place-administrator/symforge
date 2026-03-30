#!/usr/bin/env python
"""Validate commit subjects against the conventional commits format used by release-please."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ALLOWED_TYPES = (
    "build",
    "chore",
    "ci",
    "docs",
    "feat",
    "fix",
    "perf",
    "refactor",
    "revert",
    "style",
    "test",
)

IGNORED_PREFIXES = (
    "Merge pull request #",
    "Merge branch ",
    "Merge remote-tracking branch ",
)


def repo_root(path: str | None = None) -> Path:
    if path is not None:
        return Path(path).resolve()
    return Path(__file__).resolve().parent.parent


def is_ignored_subject(subject: str) -> bool:
    return subject.startswith(IGNORED_PREFIXES)


def is_conventional_subject(subject: str) -> bool:
    for commit_type in ALLOWED_TYPES:
        if not subject.startswith(commit_type):
            continue

        remainder = subject[len(commit_type) :]
        if remainder.startswith("("):
            closing = remainder.find(")")
            if closing <= 1:
                return False
            remainder = remainder[closing + 1 :]

        if remainder.startswith("!"):
            remainder = remainder[1:]

        return remainder.startswith(": ") and len(remainder) > 2

    return False


def check_subjects(subjects: list[str]) -> list[str]:
    problems: list[str] = []

    for subject in subjects:
        if is_ignored_subject(subject):
            continue
        if not is_conventional_subject(subject):
            allowed = ", ".join(ALLOWED_TYPES)
            problems.append(
                f"'{subject}' is not a conventional commit subject. "
                f"Expected one of: {allowed}. Example: fix(ci): describe the change"
            )

    return problems


def read_commit_subjects(root: Path, rev_range: str) -> list[str]:
    result = subprocess.run(
        ["git", "log", "--format=%s", rev_range],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"git log failed for range '{rev_range}'")
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate commit subjects against release-please-friendly conventional commits."
    )
    parser.add_argument(
        "--root",
        default=None,
        help="Repository root to inspect. Defaults to the current repo.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_subject = subparsers.add_parser(
        "check-subject",
        help="Validate one commit subject or PR title.",
    )
    check_subject.add_argument("subject", help="Subject/title to validate.")

    check_range = subparsers.add_parser(
        "check-range",
        help="Validate every commit subject in a git revision range.",
    )
    check_range.add_argument("rev_range", help="Git revision range, for example HEAD~3..HEAD.")

    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = repo_root(args.root)

    try:
        if args.command == "check-subject":
            problems = check_subjects([args.subject])
        elif args.command == "check-range":
            subjects = read_commit_subjects(root, args.rev_range)
            if not subjects:
                print(f"No commits found in range {args.rev_range}.")
                return 0
            problems = check_subjects(subjects)
        else:
            return 2
    except RuntimeError as error:
        print(error, file=sys.stderr)
        return 1

    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1

    if args.command == "check-subject":
        print("Conventional commit subject check passed.")
    else:
        print(f"Conventional commit range check passed: {args.rev_range}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
