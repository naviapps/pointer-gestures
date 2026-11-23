import os
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.source import source_identity


class SourceIdentityTests(unittest.TestCase):
    def test_frozen_workspace_requires_no_git_metadata(self):
        with (
            patch.dict(
                os.environ,
                {"NAVI_CI_SOURCE_COMMIT": "a" * 40, "NAVI_CI_SOURCE_FINGERPRINT": "b" * 64},
                clear=True,
            ),
            patch("subprocess.check_output", side_effect=AssertionError("Unexpected Git query")),
        ):
            self.assertEqual(source_identity(Path("source")), ("a" * 40, "b" * 64))

    def test_checkout_without_controller_context_retains_git_identity(self):
        with (
            patch.dict(os.environ, {}, clear=True),
            patch("subprocess.check_output", return_value="c" * 40 + "\n") as query,
        ):
            self.assertEqual(source_identity(Path("source")), ("c" * 40, None))
            query.assert_called_once_with(
                ["git", "rev-parse", "HEAD"], cwd=Path("source"), text=True
            )

    def test_malformed_context_is_rejected_before_native_work(self):
        for commit, fingerprint in [("invalid", "b" * 64), ("a" * 40, "invalid")]:
            with patch.dict(
                os.environ,
                {"NAVI_CI_SOURCE_COMMIT": commit, "NAVI_CI_SOURCE_FINGERPRINT": fingerprint},
                clear=True,
            ):
                with self.assertRaises(ValueError):
                    source_identity(Path("source"))
