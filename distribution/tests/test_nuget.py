"""Exercise package-workspace selection and receipt timing without running .NET."""

import json
import shutil
import tempfile
import unittest
import zipfile
from contextlib import contextmanager
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from distribution.nuget import NuGetSDK


class WindowsPackageWorkspaceTests(unittest.TestCase):
    version = "0.1.0-native.test"
    rid = "win-x64"
    library = "pointer_gesture_recognition_ffi.dll"

    def setUp(self):
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        sdk = NuGetSDK(root, "recognition", {}, "dotnet", root / "reports")
        self.sdk = sdk
        (root / "global.json").write_text(
            '{"sdk":{"version":"10.0.403","rollForward":"disable"}}',
            encoding="utf-8",
        )
        self.feed = root / "feed"
        self.feed.mkdir()
        sdk.layout.nuget_consumer.parent.mkdir(parents=True)
        sdk.layout.nuget_consumer.write_text("fixture")
        sdk.layout.ffi.mkdir(parents=True)
        (sdk.layout.ffi / "Cargo.toml").write_text(
            '[package]\nname = "pointer-gesture-recognition-ffi"\n'
        )
        self.candidate = self.feed / f"{sdk.id}.{self.version}.nupkg"
        self.documentation = f"lib/net10.0/{sdk.id}.xml"
        with zipfile.ZipFile(self.candidate, "w") as package:
            package.writestr(self.documentation, b"fixture docs")
            package.writestr(f"runtimes/{self.rid}/native/{self.library}", b"fixture DLL")
        self.receipt = sdk.report_dir / f"package-check-{self.rid}.json"
        self.enterContext(patch("distribution.windows_sdk.inspect_pe"))
        self.run_dotnet = self.enterContext(
            patch("distribution.nuget.subprocess.run", side_effect=self.dotnet_fixture)
        )

    def dotnet_fixture(self, argv, *, cwd, env, check):
        self.assertEqual(
            (cwd / "global.json").read_bytes(), (self.sdk.root / "global.json").read_bytes()
        )
        kind = argv[1]
        if kind == "restore":
            relative = f"{self.sdk.id.lower()}/{self.version}"
            installed = Path(env["NUGET_PACKAGES"]) / relative
            (installed / self.documentation).parent.mkdir(parents=True)
            shutil.copyfile(self.candidate, installed / self.candidate.name.lower())
            (installed / self.documentation).write_bytes(b"fixture docs")
            assets = cwd / "obj/project.assets.json"
            assets.parent.mkdir()
            assets.write_text(
                json.dumps(
                    {
                        "libraries": {
                            f"{self.sdk.id}/{self.version}": {
                                "path": relative,
                                "type": "package",
                            }
                        },
                    }
                )
            )
        elif kind == "build":
            output = cwd / "bin/Release/net10.0" / self.rid
            output.mkdir(parents=True)
            (output / self.library).write_bytes(b"fixture DLL")

    def consume(self, *, probe=None):
        self.sdk.package_consumer(self.rid, self.version, self.feed, execute=True, probe=probe)

    def test_receipt_waits_for_candidate_workspace_cleanup(self):
        calls = []

        @contextmanager
        def workspace(parent, *, prefix):
            with tempfile.TemporaryDirectory(dir=parent, prefix=prefix) as path:
                work = Path(path)
                calls.append(work)
                yield work
                self.assertFalse(self.receipt.exists())
                self.assertTrue((work / "native-loading" / self.library).is_file())

        def probe(work, output, env):
            native = work / "native-loading"
            native.mkdir()
            shutil.copyfile(output / self.library, native / self.library)
            return {"host-resolver": {"state": "loaded"}}

        with patch(
            "distribution.nuget.load",
            return_value=SimpleNamespace(temporary_workspace=workspace),
        ):
            self.consume(probe=probe)
        self.assertEqual(len(calls), 1)
        self.assertFalse(calls[0].exists())
        report = json.loads(self.receipt.read_text())
        self.assertEqual(report["result"], "passed")
        self.assertEqual(report["nativeLoading"], {"host-resolver": {"state": "loaded"}})

    def test_workspace_cleanup_failure_blocks_receipt(self):
        failure = PermissionError("A native loading fixture could not be removed")

        @contextmanager
        def workspace(parent, *, prefix):
            with tempfile.TemporaryDirectory(dir=parent, prefix=prefix) as path:
                yield Path(path)
                raise failure

        with (
            patch(
                "distribution.nuget.load",
                return_value=SimpleNamespace(temporary_workspace=workspace),
            ),
            self.assertRaises(PermissionError) as ctx,
        ):
            self.consume()
        self.assertIs(ctx.exception, failure)
        self.assertFalse(self.receipt.exists())
