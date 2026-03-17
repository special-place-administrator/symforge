import unittest
from unittest import mock

import release_ops


class ReleaseOpsTests(unittest.TestCase):
    def test_normalize_release_tag_adds_prefix(self) -> None:
        self.assertEqual(release_ops.normalize_release_tag("0.3.12"), "v0.3.12")

    def test_normalize_release_tag_preserves_prefix(self) -> None:
        self.assertEqual(release_ops.normalize_release_tag("v0.3.12"), "v0.3.12")

    def test_normalize_release_tag_rejects_noncanonical_shape(self) -> None:
        with self.assertRaises(release_ops.ReleaseOpsError):
            release_ops.normalize_release_tag("symforge-v0.3.12")

    def test_normalize_release_tag_rejects_blank_input(self) -> None:
        with self.assertRaises(release_ops.ReleaseOpsError):
            release_ops.normalize_release_tag("   ")

    def test_guide_text_mentions_canonical_commands(self) -> None:
        text = release_ops.guide_text()
        self.assertIn("python execution/release_ops.py preflight", text)
        self.assertIn("python execution/release_ops.py push-main", text)
        self.assertIn("python execution/release_ops.py rebuild --tag v0.3.12", text)
        self.assertIn("GitHub Actions workflow permissions", text)

    def test_parse_github_repo_slug_supports_https_and_ssh(self) -> None:
        self.assertEqual(
            release_ops.parse_github_repo_slug(
                "https://github.com/special-place-administrator/symforge.git"
            ),
            "special-place-administrator/symforge",
        )
        self.assertEqual(
            release_ops.parse_github_repo_slug(
                "git@github.com:special-place-administrator/symforge.git"
            ),
            "special-place-administrator/symforge",
        )

    def test_resolve_executable_prefers_shutil_lookup(self) -> None:
        with mock.patch("release_ops.shutil.which", return_value="C:/Tools/npm.cmd"):
            self.assertEqual(release_ops.resolve_executable("npm"), "C:/Tools/npm.cmd")

    def test_recommended_next_steps_dirty_tree_blocks_push(self) -> None:
        steps = release_ops.recommended_next_steps("main", clean=False)
        self.assertTrue(any("dirty" in step for step in steps))

    def test_preflight_steps_include_version_sync(self) -> None:
        root = release_ops.repo_root()
        rendered = [" ".join(args) for _, args, _ in release_ops.preflight_steps(root)]
        self.assertTrue(any("version_sync.py check" in command for command in rendered))


if __name__ == "__main__":
    unittest.main()
