#!/usr/bin/env python3
"""Check both gesture SDKs on native macOS or Windows hardware without publication."""

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

from tools._tooling import add_candidate_arguments, candidate_arguments, load, verify
from tools.sdk import KINDS, SDK
from tools.source import source_identity
from tools.windows import RID, TARGET, WindowsSDK


def retain_verified_packages(root, feed, artifacts, rid, version):
    """Keep the native-tested feed pair under the current CI artifact root."""
    packages = {}
    receipts = {}
    system = "windows" if rid.startswith("win-") else "macos"
    execution_field = "windowsExecuted" if system == "windows" else "macOSExecuted"
    if system == "windows" and rid != RID:
        raise ValueError("Windows package retention requires win-x64")
    for kind in KINDS:
        sdk = SDK(root, kind)
        package = feed / f"{sdk.managed_id}.{version}.nupkg"
        receipt = root / f".native-build/{system}/{kind}/package-check-{rid}.json"
        if receipt.is_symlink() or not receipt.is_file():
            raise ValueError(f"Missing native package receipt: {receipt.name} for {kind}")
        report = json.loads(receipt.read_text())
        if (
            not isinstance(report, dict)
            or report.get("result") != "passed"
            or report.get(execution_field) is not True
            or report.get("rid") != rid
            or report.get("package") != package.name
            or not re.fullmatch(r"[0-9a-f]{64}", str(report.get("sha256")))
        ):
            raise ValueError(f"Native package receipt does not match {package.name} for {rid}")
        if system == "windows":
            host = report.get("hostArchitecture")
            if host not in ("arm64", "x64") or any(
                report.get(key) != value for key, value in WindowsSDK.execution(host).items()
            ):
                raise ValueError(
                    "Runtime evidence must report the Windows host and x64 execution mode"
                )
        packages[kind] = package
        receipts[kind] = report
    if {path.name for path in feed.glob("*.nupkg")} != {path.name for path in packages.values()}:
        raise ValueError("Native feed must contain exactly the two validated packages")
    if (
        system == "windows"
        and receipts["recognition"]["hostArchitecture"] != receipts["rendering"]["hostArchitecture"]
    ):
        raise ValueError("Windows package receipts disagree with the execution host")
    restored = {
        f"{SDK(root, kind).managed_id}/{version}": report["sha256"]
        for kind, report in receipts.items()
    }
    recognition_id = f"{SDK(root, 'recognition').managed_id}/{version}"
    if (
        receipts["recognition"].get("restoredPackages")
        != {recognition_id: restored[recognition_id]}
        or receipts["rendering"].get("restoredPackages") != restored
    ):
        raise ValueError("Native package receipts disagree with the installed dependency closure")

    destination = artifacts / "sdk-packages"
    if destination.exists() or destination.is_symlink():
        raise FileExistsError(f"Refusing to replace retained packages: {destination}")
    with tempfile.TemporaryDirectory(prefix="verified-nuget-", dir=artifacts) as temporary:
        staged = Path(temporary) / destination.name
        staged.mkdir()
        for kind, source in packages.items():
            if source.is_symlink() or not source.is_file():
                raise ValueError(f"Expected a regular package file: {source.name}")
            copy = staged / source.name
            shutil.copyfile(source, copy)
            with copy.open("rb") as package:
                if hashlib.file_digest(package, "sha256").hexdigest() != receipts[kind]["sha256"]:
                    raise ValueError(f"Package changed since native validation: {source.name}")
            library = SDK(root, kind).package.replace("-", "_")
            filename = library + ".dll" if system == "windows" else f"lib{library}.dylib"
            with zipfile.ZipFile(copy) as package:
                runtime_files = [
                    item.filename
                    for item in package.infolist()
                    if item.filename.startswith("runtimes/") and not item.is_dir()
                ]
            if runtime_files != [f"runtimes/{rid}/native/{filename}"]:
                raise ValueError(f"Package does not contain exactly the {rid} asset: {source.name}")
        # The Rendering package is only usable alongside this exact Recognition package.
        staged.rename(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--architecture", required=True, choices=("arm64", "x86_64"))
    parser.add_argument(
        "--desktop-input",
        action="store_true",
        help="Install global input hooks and inject into an owned isolated Windows fixture",
    )
    add_candidate_arguments(parser)
    args = parser.parse_args()
    if sys.platform != "win32" and (
        args.tooling_source is not None or args.tooling_revision is not None
    ):
        parser.error("Approved tooling selection is for Windows x64 acceptance")
    if sys.platform == "win32" and args.tooling_source is None:
        parser.error("Windows acceptance requires the approved x64 tooling checkout and revision")
    tooling_options = candidate_arguments(parser, args)
    if args.desktop_input and sys.platform != "win32":
        parser.error("--desktop-input requires Windows")
    root = Path(__file__).resolve().parent.parent
    source_commit, fingerprint = source_identity(root)
    artifacts = (
        Path(os.environ["NAVI_CI_ARTIFACTS"]) if os.environ.get("NAVI_CI_ARTIFACTS") else None
    )
    if artifacts:
        artifacts.mkdir(parents=True, exist_ok=True)
    machine = platform.machine().lower()
    machine = {"amd64": "x86_64", "aarch64": "arm64"}.get(machine, machine)
    native = os.environ.get("PROCESSOR_ARCHITEW6432") or os.environ.get(
        "PROCESSOR_ARCHITECTURE", machine
    )
    native = {"amd64": "x86_64", "aarch64": "arm64"}.get(native.lower(), native.lower())
    if machine != args.architecture or native != args.architecture:
        raise SystemExit("Run the requested architecture natively; emulation is separate evidence.")
    if sys.platform == "darwin":
        translated = subprocess.run(
            ["sysctl", "-in", "sysctl.proc_translated"], text=True, capture_output=True
        )
        if translated.stdout.strip() == "1":
            raise SystemExit("Rosetta is not native execution.")
    elif sys.platform != "win32":
        raise SystemExit("This check requires macOS or Windows.")
    env = dict(
        os.environ,
        DOTNET_CLI_TELEMETRY_OPTOUT="1",
        DOTNET_NOLOGO="1",
        DOTNET_CLI_UI_LANGUAGE="en",
        DOTNET_CLI_USE_MSBUILD_SERVER="0",
        MSBUILDDISABLENODEREUSE="1",
    )
    is_windows = sys.platform == "win32"
    if is_windows:
        sdk = WindowsSDK(root)
        sdk.env.update(env)
        host, _ = sdk.prepare_windows(msvc=True)
        env, dotnet = sdk.env, sdk.dotnet
        execution = sdk.execution(host)
    else:
        dotnet = env.get("DOTNET_BIN") or shutil.which("dotnet")
        if not dotnet:
            raise SystemExit("Install a native .NET 10 SDK or set DOTNET_BIN.")
        info = subprocess.check_output([dotnet, "--info"], env=env, text=True)
        expected = "x64" if args.architecture == "x86_64" else "arm64"
        if not re.search(r"Architecture:\s+" + expected + r"\b", info):
            raise SystemExit("Select a target-native .NET SDK.")
        execution = {}

    def run(command, cwd=root):
        subprocess.run([str(value) for value in command], cwd=cwd, env=env, check=True)

    def retain_receipts():
        if not artifacts:
            return
        for kind in KINDS:
            native = root / ".native-build" / system / kind
            receipts = [(native / f"package-check-{rid}.json", "package")]
            if system == "windows":
                receipts.extend(
                    [
                        (native / rid / "native-build.json", "native"),
                        (native / f"test-{rid}" / "result.json", "c-rust"),
                    ]
                )
            for source, name in receipts:
                if source.is_file():
                    shutil.copyfile(source, artifacts / ("pointer-" + kind + "-" + name + ".json"))

    system = "windows" if is_windows else "macos"
    rid = RID if is_windows else "osx-" + expected
    if is_windows:
        run(
            [
                "cargo",
                "fetch",
                "--locked",
                "--target",
                TARGET,
            ]
        )
        notice_index = root / "ThirdParty/registry-notices.json"
        retained_index = notice_index.read_bytes()
        for operation in ("build", "test-native"):
            run([sys.executable, "-m", "tools.windows", operation, *tooling_options])
            if notice_index.read_bytes() != retained_index:
                raise SystemExit(
                    "Native validation changed the retained notice index; update and verify its source first."
                )
            retain_receipts()
    else:
        run(["make", "test-ffi"])

    if is_windows:
        run(
            [
                dotnet,
                "restore",
                "--configfile",
                "NuGet.Config",
                "-p:Configuration=Release",
                "-p:UseSharedCompilation=false",
            ],
            cwd=root / "bindings/dotnet",
        )
        for kind in KINDS:
            sdk = SDK(root, kind)
            project = sdk.tests
            load("bindings").verify_managed(
                dotnet,
                project,
                cwd=root,
                env=env,
                properties={"UseSharedCompilation": "false"},
            )
            run(
                [
                    dotnet,
                    "build",
                    project,
                    "--no-restore",
                    "-c",
                    "Release",
                    "-p:UseSharedCompilation=false",
                ]
            )
            output = project.parent / "bin/Release/net10.0"
            for dependency in KINDS:
                layout = SDK(root, dependency)
                name = layout.package.replace("-", "_")
                source = root / f".native-build/windows/{dependency}/{rid}/{name}.dll"
                destination = output / (name + ".dll")
                shutil.copyfile(source, destination)
            run([dotnet, output / (project.stem + ".dll")])

        if args.desktop_input:
            run([sys.executable, "-m", "tools.windows", "test-input", *tooling_options])
        project = root / "bindings/dotnet/examples/windows"
        run(
            [
                dotnet,
                "restore",
                project,
                "--source",
                "https://api.nuget.org/v3/index.json",
                "-p:NativeRid=" + rid,
                "-p:Configuration=Release",
                "-p:UseSharedCompilation=false",
            ]
        )
        load("bindings").verify_managed(
            dotnet,
            project,
            cwd=root,
            env=env,
            properties={"NativeRid": rid, "UseSharedCompilation": "false"},
        )
        if args.desktop_input:
            with tempfile.TemporaryDirectory(prefix="pointer-native-ui-") as directory:
                try:
                    run(
                        [
                            dotnet,
                            "run",
                            "--no-restore",
                            "--project",
                            project,
                            "-c",
                            "Release",
                            "-p:NativeRid=" + rid,
                            "-p:UseSharedCompilation=false",
                            "--",
                            "--verify",
                            directory,
                        ]
                    )
                finally:
                    report = Path(directory) / "result.json"
                    if report.exists():
                        print(report.read_text())
                    if artifacts:
                        shutil.copytree(directory, artifacts / "pointer-gestures-ui")
        else:
            run(
                [
                    dotnet,
                    "build",
                    project,
                    "--no-restore",
                    "-c",
                    "Release",
                    "-p:NativeRid=" + rid,
                    "-p:UseSharedCompilation=false",
                ]
            )
    # Pack the same runtime that passed the C and managed fixtures.
    version = "0.1.0-native." + (fingerprint or source_commit)[:12]
    with tempfile.TemporaryDirectory(prefix="pointer-native-feed-") as directory:
        run(
            [
                sys.executable,
                "-m",
                "tools.nuget",
                "pack",
                "--rid",
                rid,
                "--version",
                version,
                "--output",
                directory,
                *tooling_options,
            ]
        )
        run(
            [
                sys.executable,
                "-m",
                "tools.nuget",
                "test-package",
                "--rid",
                rid,
                "--version",
                version,
                "--feed",
                directory,
                *tooling_options,
            ]
        )
        if is_windows:
            verify(args.tooling_source, args.tooling_revision)
        if artifacts:
            retain_verified_packages(root, Path(directory), artifacts, rid, version)
    result = {
        "result": "passed",
        "system": platform.platform(),
        "architecture": machine,
        "runtime": rid,
        **execution,
        "toolingRevision": args.tooling_revision,
        "source": source_commit,
        "sourceFingerprint": fingerprint,
        "desktopInput": args.desktop_input,
        "scope": "C interop and clean .NET package consumers. Windows also checks Rust and managed sources. Desktop-input opt-in exercises the owned receiver, WinForms overlay and native input.",
    }
    if artifacts:
        retain_receipts()
        (artifacts / f"pointer-gestures-{system}.json").write_text(
            json.dumps(result, indent=2) + "\n"
        )
    print(json.dumps(result))


if __name__ == "__main__":
    main()
