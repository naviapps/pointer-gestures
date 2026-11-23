"""Exercise the SDK's Windows toolchain routing, export and import policies."""

import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from fixtures.distribution import windows


class PointerExportPolicyTests(unittest.TestCase):
    def test_each_library_requires_its_portable_and_windows_header_exports(self):
        for kind in ("recognition", "rendering"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                include = root / "crates" / (kind + "-ffi") / "include"
                platform = include / "windows"
                platform.mkdir(parents=True)
                prefix = f"pointer_gesture_{kind}_"
                (include / "portable.h").write_text(
                    f"uint32_t {prefix}abi_version(void);\nvoid unrelated_function(void);\n"
                )
                (platform / "native.h").write_text(f"uint32_t {prefix}windows_abi_version(void);\n")
                with patch.object(
                    windows, "builder", return_value=SimpleNamespace(package="ffi", library="ffi")
                ):
                    sdk = windows.WindowsSDK(root, kind)
                self.assertEqual(
                    sdk.symbols, {prefix + "abi_version", prefix + "windows_abi_version"}
                )


class PointerDLLPolicyTests(unittest.TestCase):
    def accepted(self, name):
        def inspector(path, machine, symbols, *, allow_import):
            if not allow_import(name):
                raise ValueError("Unexpected runtime dependency: " + name)
            return {"imports": [name]}

        with patch.object(windows, "load", return_value=SimpleNamespace(inspect_pe=inspector)):
            return windows.inspect_pe(
                Path("owned.dll"), windows.MACHINE, {"pointer_gesture_recognition_test"}
            )

    def test_system_rng_and_windows_api_sets_remain_allowed(self):
        for name in (
            "kernel32.dll",
            "gdi32.dll",
            "bcryptprimitives.dll",
            "api-ms-win-core-file-l1-1-0.dll",
        ):
            self.assertEqual(self.accepted(name)["imports"], [name])

    def test_external_crt_or_similarly_named_dll_is_not_allowed(self):
        for name in (
            "vcruntime140.dll",
            "surprise.dll",
            "bcryptprimitives-extra.dll",
            "api-ms-win-crt-heap-l1-1-0.dll",
        ):
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.accepted(name)


class WindowsTargetSelectionTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        (self.root / "mise.toml").write_text('[tools]\ndotnet = "10.0.403"\n')
        self.enterContext(
            patch.object(
                windows, "builder", return_value=SimpleNamespace(package="ffi", library="ffi")
            )
        )
        self.sdk = windows.WindowsSDK(self.root)

    def test_native_checks_require_windows(self):
        with (
            patch.object(windows.sys, "platform", "darwin"),
            patch.object(windows, "load") as load,
        ):
            with self.assertRaisesRegex(ValueError, "requires a Windows worker"):
                self.sdk.prepare_windows()
            load.assert_not_called()

    def test_arm64_host_prepares_only_x64_build_and_runtime_consumers(self):
        compiler = self.root / "msvc"
        dotnet = str(self.root / "dotnet-x64" / "dotnet.exe")
        shared = Mock()
        shared.host_architecture.return_value = "arm64"
        shared.native_msvc_environment.side_effect = lambda host, env, **_: (env, compiler)
        shared.native_environment.side_effect = lambda host, env, **_: (env, compiler, dotnet)
        shared.target_dotnet.return_value = dotnet
        check_output = Mock(return_value="10.0.403\n")
        with (
            patch.object(windows.sys, "platform", "win32"),
            patch.object(windows, "load", return_value=shared),
            patch.object(windows.subprocess, "check_output", check_output),
        ):
            self.sdk.build()
            call = shared.native_msvc_environment.call_args
            self.assertEqual(call.args[0], "arm64")
            self.assertEqual(call.kwargs, {"target_architecture": "x64"})
            build = shared.build_library.call_args.kwargs
            self.assertEqual(
                (build["architecture"], build["native_host"], build["tools"]),
                ("x64", "arm64", compiler),
            )
            shared.target_dotnet.assert_not_called()
            shared.native_environment.assert_not_called()
            check_output.assert_not_called()
            self.assertEqual(self.sdk.prepare_windows(msvc=True), ("arm64", compiler))
            self.assertEqual(
                shared.native_environment.call_args.kwargs, {"target_architecture": "x64"}
            )
            self.assertEqual(self.sdk.prepare_windows(), ("arm64", None))
            self.assertEqual(shared.target_dotnet.call_args.args[:2], ("arm64", "x64"))
            self.assertEqual(self.sdk.nuget.dotnet, dotnet)
            check_output.return_value = "10.0.402\n"
            with self.assertRaisesRegex(ValueError, "pinned x64 .NET SDK"):
                self.sdk.prepare_windows()

    def test_cli_rejects_an_arm64_runtime(self):
        with (
            patch.object(windows.sys, "argv", ["windows", "verify", "--rid", "win-arm64"]),
            patch.object(windows, "load") as load,
            patch("sys.stderr"),
        ):
            with self.assertRaises(SystemExit) as exit:
                windows.main()
            self.assertEqual(exit.exception.code, 2)
            load.assert_not_called()


if __name__ == "__main__":
    unittest.main()
