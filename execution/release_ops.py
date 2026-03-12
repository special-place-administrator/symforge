#!/usr/bin/env python
"""Operator entrypoints for Tokenizor release and publish workflow."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path


class ReleaseOpsError(RuntimeError):
    """Raised when a release operation cannot be completed safely."""


def repo_root(path: str | None = None) -> Path:
    if path is not None:
        return Path(path).resolve()
    return Path(__file__).resolve().parent.parent


def normalize_release_tag(tag: str) -> str:
    cleaned = tag.strip()
    if not cleaned:
        raise ReleaseOpsError("release tag must not be empty")
    return cleaned if cleaned.startswith("v") else f"v{cleaned}"


def guide_text() -> str:
    return """Tokenizor release operator guide

Fresh terminal commands:
  python execution/release_ops.py status
  python execution/release_ops.py preflight
  python execution/release_ops.py push-main

Normal publish flow:
  1. Make sure your branch is `main` and the working tree is clean.
  2. Run `python execution/release_ops.py preflight`.
  3. Run `python execution/release_ops.py push-main`.
  4. Wait for the release PR opened by `release-please`.
  5. Merge that release PR on GitHub.
  6. GitHub Actions builds binaries, uploads release assets, and publishes npm.

Recovery flow for an existing tag:
  python execution/release_ops.py rebuild --tag v0.3.12

Source of truth:
  - docs/release-process.md
  - .github/workflows/release.yml
  - execution/version_sync.py
"""


def recommended_next_steps(branch: str, clean: bool) -> list[str]:
    if branch != "main":
        return [
            f"Current branch is '{branch}'. Switch to 'main' before running push-main.",
            "If you only need a reminder of the release flow, run `python execution/release_ops.py guide`.",
        ]
    if not clean:
        return [
            "Working tree is dirty. Commit or stash changes before running push-main.",
            "When the tree is clean, run `python execution/release_ops.py preflight`.",
        ]
    return [
        "Branch and working tree are ready for release preflight.",
        "Next commands: `python execution/release_ops.py preflight` then `python execution/release_ops.py push-main`.",
    ]


def run_checked(
    args: list[str],
    *,
    cwd: Path,
    capture_output: bool = False,
) -> str:
    completed = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        capture_output=capture_output,
        check=False,
    )
    if completed.returncode != 0:
        rendered = " ".join(args)
        message = f"command failed: {rendered}"
        if capture_output:
            stderr = completed.stderr.strip()
            stdout = completed.stdout.strip()
            detail = stderr or stdout
            if detail:
                message = f"{message}\n{detail}"
        raise ReleaseOpsError(message)
    return completed.stdout.strip() if capture_output else ""


def try_capture(args: list[str], *, cwd: Path) -> str | None:
    completed = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        return None
    return completed.stdout.strip()


def current_branch(root: Path) -> str:
    return run_checked(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=root, capture_output=True)


def git_is_clean(root: Path) -> bool:
    return run_checked(["git", "status", "--short"], cwd=root, capture_output=True) == ""


def current_version(root: Path) -> str:
    return run_checked(
        [sys.executable, str(root / "execution" / "version_sync.py"), "current"],
        cwd=root,
        capture_output=True,
    )


def release_metadata_is_aligned(root: Path) -> bool:
    completed = subprocess.run(
        [sys.executable, str(root / "execution" / "version_sync.py"), "check"],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    return completed.returncode == 0


def preflight_steps(root: Path) -> list[tuple[str, list[str], Path]]:
    return [
        (
            "Verify release metadata alignment",
            [sys.executable, str(root / "execution" / "version_sync.py"), "check"],
            root,
        ),
        (
            "Run execution unit tests",
            [sys.executable, "-m", "unittest", "discover", "-s", "execution", "-p", "test_*.py"],
            root,
        ),
        ("Run npm tests", ["npm", "test"], root / "npm"),
        ("Check Rust formatting", ["cargo", "fmt", "--all", "--check"], root),
        ("Run Rust tests", ["cargo", "test", "--all-targets", "--", "--test-threads=1"], root),
    ]


def run_preflight(root: Path) -> None:
    for label, args, cwd in preflight_steps(root):
        print(f"==> {label}")
        run_checked(args, cwd=cwd)


def cmd_guide(args: argparse.Namespace) -> int:
    _ = args
    print(guide_text())
    return 0


def cmd_status(args: argparse.Namespace) -> int:
    root = repo_root(args.root)
    branch = current_branch(root)
    clean = git_is_clean(root)
    version = current_version(root)
    aligned = release_metadata_is_aligned(root)
    upstream = try_capture(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"], cwd=root)
    latest_tag = try_capture(["git", "describe", "--tags", "--abbrev=0"], cwd=root)

    print(f"Repo root: {root}")
    print(f"Branch: {branch}")
    print(f"Working tree: {'clean' if clean else 'dirty'}")
    print(f"Canonical version: {version}")
    print(f"Release metadata: {'aligned' if aligned else 'drifted'}")
    if upstream:
        print(f"Upstream: {upstream}")
    if latest_tag:
        print(f"Latest tag: {latest_tag}")
    print("")
    for line in recommended_next_steps(branch, clean):
        print(line)
    return 0


def cmd_preflight(args: argparse.Namespace) -> int:
    root = repo_root(args.root)
    run_preflight(root)
    print("Release preflight passed.")
    return 0


def cmd_push_main(args: argparse.Namespace) -> int:
    root = repo_root(args.root)
    branch = current_branch(root)
    if branch != "main":
        raise ReleaseOpsError(f"refusing to push: current branch is '{branch}', expected 'main'")
    if not git_is_clean(root):
        raise ReleaseOpsError("refusing to push: working tree is dirty")
    if not args.skip_preflight:
        run_preflight(root)
    print("==> Pushing main")
    run_checked(["git", "push", "origin", "main"], cwd=root)
    print("Push complete. If a release is due, release-please will open or update the release PR.")
    return 0


def cmd_rebuild(args: argparse.Namespace) -> int:
    root = repo_root(args.root)
    tag = normalize_release_tag(args.tag)
    gh = shutil.which("gh")
    if gh is None:
        raise ReleaseOpsError(
            "GitHub CLI 'gh' is required for rebuild dispatch. "
            f"Manual command: gh workflow run Release --ref main -f tag={tag}"
        )
    run_checked([gh, "workflow", "run", "Release", "--ref", "main", "-f", f"tag={tag}"], cwd=root)
    print(f"Triggered Release workflow rebuild for {tag}.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Canonical operator commands for Tokenizor release and publish workflow."
    )
    parser.add_argument(
        "--root",
        default=None,
        help="Repository root to operate on. Defaults to the current repository.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    guide = subparsers.add_parser("guide", help="Print the release operator runbook.")
    guide.set_defaults(func=cmd_guide)

    status = subparsers.add_parser("status", help="Show current repo release readiness.")
    status.set_defaults(func=cmd_status)

    preflight = subparsers.add_parser("preflight", help="Run the local release preflight checks.")
    preflight.set_defaults(func=cmd_preflight)

    push_main = subparsers.add_parser(
        "push-main",
        help="Run preflight and push the current main branch to origin.",
    )
    push_main.add_argument(
        "--skip-preflight",
        action="store_true",
        help="Push without rerunning preflight checks.",
    )
    push_main.set_defaults(func=cmd_push_main)

    rebuild = subparsers.add_parser(
        "rebuild",
        help="Trigger the GitHub Release workflow for an existing tag.",
    )
    rebuild.add_argument("--tag", required=True, help="Existing release tag, for example v0.3.12.")
    rebuild.set_defaults(func=cmd_rebuild)

    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except ReleaseOpsError as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
