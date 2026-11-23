"""Exercise CI Web startup before any compiler or network operation."""

import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "ci_web_startup", ROOT / ".github/scripts/check-web.py"
)
CI_WEB = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CI_WEB)


class RuntimeBoundaryReached(Exception):
    pass


class PreparedToolsBoundary:
    def is_absolute(self):
        return True

    def resolve(self, *, strict):
        return self

    def __truediv__(self, _relative):
        raise RuntimeBoundaryReached


class WebArtifactDirectoryTests(unittest.TestCase):
    def start(self, base):
        tools = str(base / "prepared-tools")

        def path(value):
            return PreparedToolsBoundary() if str(value) == tools else Path(value)

        environment = {
            "NAVI_CI_TOOLS_ROOT": tools,
            "RUNNER_TEMP": str(base / "temporary"),
            "NAVI_CI_ARTIFACTS": str(base / "artifacts"),
        }
        with mock.patch.dict(os.environ, environment), mock.patch.object(CI_WEB, "Path", path):
            CI_WEB.main()

    def test_creates_missing_artifacts_parent(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            with self.assertRaises(RuntimeBoundaryReached):
                self.start(base)
            self.assertTrue((base / "artifacts/web").is_dir())

    def test_preserves_other_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            artifacts = base / "artifacts"
            artifacts.mkdir()
            previous = artifacts / "other-step.log"
            previous.write_bytes(b"owned earlier result")
            with self.assertRaises(RuntimeBoundaryReached):
                self.start(base)
            self.assertEqual(previous.read_bytes(), b"owned earlier result")
            self.assertTrue((artifacts / "web").is_dir())

    def test_rejects_occupied_web_output_without_overwriting(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            output = base / "artifacts/web"
            output.mkdir(parents=True)
            previous = output / "result.log"
            previous.write_bytes(b"keep")
            with self.assertRaises(FileExistsError):
                self.start(base)
            self.assertEqual(previous.read_bytes(), b"keep")

    def test_rejects_file_as_artifact_root(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            artifact = base / "artifacts"
            artifact.write_bytes(b"keep")
            with self.assertRaises((FileExistsError, NotADirectoryError)):
                self.start(base)
            self.assertEqual(artifact.read_bytes(), b"keep")


if __name__ == "__main__":
    unittest.main()
