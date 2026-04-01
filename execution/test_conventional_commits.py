from __future__ import annotations

import subprocess
import unittest
import uuid
from pathlib import Path

import conventional_commits


class ConventionalCommitTests(unittest.TestCase):
    def test_accepts_basic_conventional_subject(self) -> None:
        self.assertEqual(conventional_commits.check_subjects(["fix: handle daemon proxy drift"]), [])

    def test_accepts_scoped_breaking_subject(self) -> None:
        self.assertEqual(
            conventional_commits.check_subjects(["feat(cli)!: require explicit project root"]),
            [],
        )

    def test_accepts_release_chore_subject(self) -> None:
        self.assertEqual(
            conventional_commits.check_subjects(["chore(main): release 4.9.6"]),
            [],
        )

    def test_ignores_merge_commit_subjects(self) -> None:
        self.assertEqual(
            conventional_commits.check_subjects(
                ["Merge pull request #186 from special-place-administrator/release-please"]
            ),
            [],
        )

    def test_rejects_nonconventional_subject(self) -> None:
        problems = conventional_commits.check_subjects(["Add conformance suite for MCP tool surface"])
        self.assertEqual(len(problems), 1)
        self.assertIn("not a conventional commit subject", problems[0])

    def test_read_commit_subjects_from_range(self) -> None:
        root = self.make_repo()
        self.git(root, "init")
        self.git(root, "config", "user.name", "Hermes")
        self.git(root, "config", "user.email", "hermes@example.com")

        (root / "README.md").write_text("one\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "fix: first")

        (root / "README.md").write_text("two\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "feat: second")

        subjects = conventional_commits.read_commit_subjects(root, "HEAD~1..HEAD")
        self.assertEqual(subjects, ["feat: second"])

    def test_resolve_push_range_uses_before_after_when_ancestor(self) -> None:
        root = self.make_repo()
        self.git(root, "init")
        self.git(root, "config", "user.name", "Hermes")
        self.git(root, "config", "user.email", "hermes@example.com")

        (root / "README.md").write_text("one\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "fix: first")
        before = self.git_stdout(root, "rev-parse", "HEAD")

        (root / "README.md").write_text("two\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "feat: second")
        after = self.git_stdout(root, "rev-parse", "HEAD")

        rev_range, note = conventional_commits.resolve_push_range(root, before, after)
        self.assertEqual(rev_range, f"{before}..{after}")
        self.assertIsNone(note)

    def test_resolve_push_range_falls_back_when_before_missing(self) -> None:
        root = self.make_repo()
        self.git(root, "init")
        self.git(root, "config", "user.name", "Hermes")
        self.git(root, "config", "user.email", "hermes@example.com")

        (root / "README.md").write_text("one\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "fix: first")
        after = self.git_stdout(root, "rev-parse", "HEAD")

        missing_before = "dcd7f495a913fa5dbe36f3311dc9bb175c6acd49"
        rev_range, note = conventional_commits.resolve_push_range(root, missing_before, after)
        self.assertEqual(rev_range, f"{after}^!")
        self.assertIsNotNone(note)
        self.assertIn("likely force-push", note)

    def test_resolve_push_range_falls_back_when_before_is_not_ancestor(self) -> None:
        root = self.make_repo()
        self.git(root, "init")
        self.git(root, "config", "user.name", "Hermes")
        self.git(root, "config", "user.email", "hermes@example.com")

        (root / "README.md").write_text("one\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "fix: base")
        base = self.git_stdout(root, "rev-parse", "HEAD")

        self.git(root, "checkout", "-b", "topic")
        (root / "README.md").write_text("topic\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "feat: topic")
        before = self.git_stdout(root, "rev-parse", "HEAD")

        self.git(root, "checkout", "master")
        (root / "README.md").write_text("main\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(root, "commit", "-m", "fix: mainline")
        after = self.git_stdout(root, "rev-parse", "HEAD")

        self.assertNotEqual(base, before)
        rev_range, note = conventional_commits.resolve_push_range(root, before, after)
        self.assertEqual(rev_range, f"{after}^!")
        self.assertIsNotNone(note)
        self.assertIn("not an ancestor", note)

    def make_repo(self) -> Path:
        temp_root = Path(__file__).resolve().parent.parent / ".tmp" / "execution-tests"
        temp_root.mkdir(parents=True, exist_ok=True)
        root = temp_root / f"repo-{uuid.uuid4().hex}"
        root.mkdir()
        return root

    def git(self, root: Path, *args: str) -> None:
        subprocess.run(["git", *args], cwd=root, check=True, capture_output=True, text=True)

    def git_stdout(self, root: Path, *args: str) -> str:
        return subprocess.run(
            ["git", *args],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()


if __name__ == "__main__":
    unittest.main()
