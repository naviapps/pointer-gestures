"""Verify that native package retention keeps only the tested pair."""

import hashlib
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from tools.native_host import retain_verified_packages
from tools.windows import WindowsSDK


class NativePackageRetentionTests(unittest.TestCase):
    rid = "win-x64"
    foreign_rid = "win-arm64"
    system = "windows"
    execution_field = "windowsExecuted"
    version = "0.1.0-native.123456789abc"

    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.feed = self.root / "feed"
        self.feed.mkdir()
        self.artifacts = self.root / "artifacts"
        self.artifacts.mkdir()
        self.destination = self.artifacts / "sdk-packages"
        self.packages = {}
        self.receipts = {}
        self.restored = {}
        for kind in ("recognition", "rendering"):
            sdk_id = f"NaviApps.PointerGestures.{kind.title()}"
            crate = self.root / "crates" / f"{kind}-ffi"
            crate.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(f'[package]\nname = "pointer-gesture-{kind}-ffi"\n')
            self.packages[kind] = self.feed / f"{sdk_id}.{self.version}.nupkg"
            self.write_package(kind, self.rid)
            receipt = (
                self.root / ".native-build" / self.system / kind / f"package-check-{self.rid}.json"
            )
            receipt.parent.mkdir(parents=True)
            self.receipts[kind] = receipt
            self.restored[f"{sdk_id}/{self.version}"] = self.package_hash(kind)
            self.write_receipt(kind, restoredPackages=self.restored)

    def package_hash(self, kind):
        return hashlib.sha256(self.packages[kind].read_bytes()).hexdigest()

    def write_package(self, kind, rid):
        library = f"pointer_gesture_{kind}_ffi"
        filename = f"{library}.dll" if self.system == "windows" else f"lib{library}.dylib"
        with zipfile.ZipFile(self.packages[kind], "w") as archive:
            archive.writestr(f"runtimes/{rid}/native/{filename}", kind)

    def write_receipt(self, kind, **overrides):
        self.receipts[kind].write_text(
            json.dumps(
                {
                    "result": "passed",
                    self.execution_field: True,
                    "rid": self.rid,
                    "package": self.packages[kind].name,
                    "sha256": self.package_hash(kind),
                    **(WindowsSDK.execution("arm64") if self.system == "windows" else {}),
                    **overrides,
                }
            )
        )

    def retain(self):
        retain_verified_packages(self.root, self.feed, self.artifacts, self.rid, self.version)

    def test_retains_both_validated_archives_without_repacking(self):
        self.retain()
        self.assertEqual(
            {file.name: file.read_bytes() for file in self.destination.iterdir()},
            {file.name: file.read_bytes() for file in self.packages.values()},
        )
        with self.assertRaises(FileExistsError):
            self.retain()

    def test_requires_both_matching_native_execution_receipts(self):
        for override in (
            {"result": "failed"},
            {self.execution_field: False},
            {"rid": self.foreign_rid},
            {"package": self.packages["recognition"].name},
            {"restoredPackages": {**self.restored, next(iter(self.restored)): "a" * 64}},
        ):
            with self.subTest(override=override):
                self.write_receipt("rendering", **({"restoredPackages": self.restored} | override))
                with self.assertRaises(ValueError):
                    self.retain()
                self.assertEqual(list(self.artifacts.iterdir()), [])
        self.receipts["rendering"].unlink()
        with self.assertRaises(ValueError):
            self.retain()
        self.assertEqual(list(self.artifacts.iterdir()), [])

    def test_rejects_inconsistent_windows_execution_evidence(self):
        if self.system != "windows":
            return
        for overrides in (
            {"targetArchitecture": "arm64"},
            {"executionMode": "native"},
            WindowsSDK.execution("x64"),
        ):
            with self.subTest(overrides=overrides):
                self.write_receipt("rendering", restoredPackages=self.restored, **overrides)
                with self.assertRaisesRegex(ValueError, "execution (mode|host)"):
                    self.retain()
                self.assertEqual(list(self.artifacts.iterdir()), [])

    def test_rejects_changed_second_archive_without_leaving_first(self):
        with self.packages["rendering"].open("ab") as archive:
            archive.write(b"changed after validation")
        with self.assertRaisesRegex(ValueError, "changed since native validation"):
            self.retain()
        self.assertEqual(list(self.artifacts.iterdir()), [])

    def test_rejects_foreign_runtime_even_when_archive_hash_matches_receipt(self):
        self.write_package("rendering", self.foreign_rid)
        restored = {
            **self.restored,
            f"NaviApps.PointerGestures.Rendering/{self.version}": self.package_hash("rendering"),
        }
        self.write_receipt("rendering", restoredPackages=restored)
        with self.assertRaisesRegex(ValueError, f"does not contain exactly the {self.rid} asset"):
            self.retain()
        self.assertEqual(list(self.artifacts.iterdir()), [])

    def test_rejects_an_unvalidated_feed_entry(self):
        (self.feed / "unexpected.nupkg").write_bytes(b"extra package")
        with self.assertRaisesRegex(ValueError, "exactly the two validated packages"):
            self.retain()
        self.assertEqual(list(self.artifacts.iterdir()), [])


class MacOSNativePackageRetentionTests(NativePackageRetentionTests):
    rid = "osx-arm64"
    foreign_rid = "osx-x64"
    system = "macos"
    execution_field = "macOSExecuted"
