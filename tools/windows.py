#!/usr/bin/env python3
"""Local Windows MSVC/.NET SDK build, inspection and package round-trip."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

from tools._tooling import add_candidate_arguments, candidate_arguments, candidate_selected, load
from tools.native import builder
from tools.nuget import NuGetSDK
from tools.sdk import KINDS
from tools.sdk import SDK as SDKLayout

RID = "win-x64"
TARGET = "x86_64-pc-windows-msvc"
MACHINE = 0x8664
HOST_TARGETS = {"x64": TARGET, "arm64": "aarch64-pc-windows-msvc"}
# ProcessPrng uses Windows' BCryptPrimitives; external CRT redistributables stay excluded.
SYSTEM_DLLS = {
    "kernel32.dll",
    "kernelbase.dll",
    "ntdll.dll",
    "advapi32.dll",
    "bcrypt.dll",
    "bcryptprimitives.dll",
    "ws2_32.dll",
    "userenv.dll",
    "synchronization.dll",
    "ole32.dll",
    "shell32.dll",
    "user32.dll",
    "gdi32.dll",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inspect_pe(path, machine, symbols):
    def allowed(name):
        return (
            name in SYSTEM_DLLS or name.startswith(("api-ms-win-", "ext-ms-win-"))
        ) and not name.startswith("api-ms-win-crt-")

    return load("windows").inspect_pe(path, machine, symbols, allow_import=allowed)


def public_exports(layout):
    prefix = layout.kind
    return set(
        re.findall(
            rf"\b(pointer_gesture_{prefix}_\w+)\s*\(",
            "\n".join(p.read_text() for p in (layout.ffi / "include").rglob("*.h")),
        )
    )


class WindowsSDK:
    def __init__(self, root, kind="recognition"):
        self.root = root.resolve()
        self.layout = SDKLayout(self.root, kind)
        self.ffi = self.layout.ffi
        self.builder = builder(self.root, kind)
        self.package = self.builder.package
        self.library = self.builder.library
        self.symbols = public_exports(self.layout)
        self.csproj = self.layout.project
        self.id = self.csproj.stem
        self.native = self.root / ".native-build/windows" / kind
        self.env = dict(
            os.environ,
            RUSTUP_AUTO_INSTALL="0",
            CARGO_PROFILE_RELEASE_STRIP="none",
            DOTNET_CLI_TELEMETRY_OPTOUT="1",
            DOTNET_CLI_UI_LANGUAGE="en",
            DOTNET_CLI_USE_MSBUILD_SERVER="0",
            MSBUILDDISABLENODEREUSE="1",
        )
        self.dotnet = self.env.get("DOTNET_BIN") or shutil.which("dotnet") or "dotnet"
        self.nuget = NuGetSDK(self.root, kind, self.env, self.dotnet, self.native)

    def run(self, args, **kwargs):
        subprocess.run([str(a) for a in args], cwd=self.root, env=self.env, check=True, **kwargs)

    def sources(self):
        policy = [
            self.root / "tools" / name
            for name in ("windows.py", "native.py", "sdk.py", "_tooling.py")
        ]
        return dict(self.builder.build_sources(), **self.builder.hashes(policy, self.root))

    def rust(self):
        compiler = subprocess.check_output(
            ["rustc", "--version", "--verbose"], cwd=self.root, env=self.env, text=True
        )
        if not compiler.startswith("rustc " + self.builder.compiler + " "):
            raise ValueError("Preinstall pinned Rust " + self.builder.compiler)
        return compiler

    def check(self, check_rust=True):
        self.rust()
        if check_rust:
            self.run(
                [
                    "cargo",
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--offline",
                    "--target",
                    TARGET,
                ]
            )
        clang = shutil.which("clang")
        if clang:
            self.run(
                [
                    clang,
                    "--target=" + TARGET,
                    "-std=c11",
                    "-ffreestanding",
                    "-fsyntax-only",
                    "-I" + str(self.ffi / "include"),
                    self.ffi / "tests/c/layout.c",
                ]
            )
        else:
            print(
                "Clang unavailable: cross-target C layouts not checked. Run test-native on each Windows target."
            )
        self.run(
            [
                self.dotnet,
                "restore",
                self.csproj,
                "--configfile",
                self.root / "bindings/dotnet/NuGet.Config",
                "-p:Configuration=Release",
                "-p:UseSharedCompilation=false",
            ]
        )
        load("bindings").verify_managed(
            self.dotnet,
            self.csproj,
            cwd=self.root,
            env=self.env,
            properties={"UseSharedCompilation": "false"},
        )
        self.run(
            [
                self.dotnet,
                "build",
                self.csproj,
                "--no-restore",
                "-c",
                "Release",
                "-p:UseSharedCompilation=false",
            ]
        )
        print(f"Portable checks passed for {RID}. No Windows DLL linked or executed.")

    def prepare_windows(self, *, msvc=False, managed=True):
        if sys.platform != "win32":
            raise ValueError("This check requires a Windows worker")
        if not candidate_selected():
            raise ValueError("Use the approved x64 tooling checkout and revision for native checks")
        windows = load("windows")
        host = windows.host_architecture()
        if host not in HOST_TARGETS:
            raise ValueError("Use native ARM64 or x64 Windows tools")
        env = self.env.copy()
        tools = None
        if msvc and managed:
            env, tools, dotnet = windows.native_environment(host, env, target_architecture="x64")
        elif msvc:
            env, tools = windows.native_msvc_environment(host, env, target_architecture="x64")
        else:
            dotnet = windows.target_dotnet(host, "x64", env)
        self.env = env
        if managed:
            sdk_version = tomllib.loads((self.root / "mise.toml").read_text())["tools"]["dotnet"]
            if (
                subprocess.check_output([dotnet, "--version"], env=env, text=True).strip()
                != sdk_version
            ):
                raise ValueError("Use the pinned x64 .NET SDK")
            self.dotnet = dotnet
            self.nuget = NuGetSDK(self.root, self.layout.kind, env, dotnet, self.native)
        return host, tools

    @staticmethod
    def execution(host):
        return {
            "hostArchitecture": host,
            "targetArchitecture": "x64",
            "executionMode": "x64-emulation" if host == "arm64" else "native",
        }

    def build(self, cross=False):
        if cross:
            native_selection = {}
        else:
            host, tools = self.prepare_windows(msvc=True, managed=False)
            native_selection = {"native_host": host, "tools": tools}

        def stage(stage):
            shutil.copytree(self.ffi / "include", stage / "include")
            shutil.copytree(
                self.root / "ThirdParty",
                stage / "licenses",
                ignore=shutil.ignore_patterns(".DS_Store", "registry-notices.json"),
            )
            shutil.copyfile(self.root / "LICENSE", stage / "licenses/LICENSE")

        load("windows").build_library(
            self.builder,
            self.env,
            architecture="x64",
            output_root=self.native,
            cross=cross,
            source_inputs=self.sources,
            inspect=lambda path: inspect_pe(path, MACHINE, self.symbols),
            stage_extra=stage,
            **native_selection,
        )
        print(f"Built and inspected {RID}. Windows execution is a separate check.")

    def verify(self):
        directory, _ = load("windows").verify_artifact(
            self.builder,
            "x64",
            output_root=self.native,
            source_inputs=self.sources,
            environment=self.env,
            inspect=lambda path: inspect_pe(path, MACHINE, self.symbols),
        )
        return directory

    def test_native(self):
        host, tools = self.prepare_windows(msvc=True, managed=False)
        directory = self.verify()
        if f"host: {HOST_TARGETS[host]}" not in self.rust().splitlines():
            raise ValueError("Use the pinned Windows Rust compiler for the host architecture")
        work = self.native / ("test-" + RID)
        work.mkdir(parents=True, exist_ok=True)
        program = work / "ownership.exe"
        self.run(
            [
                tools / "cl.exe",
                "/nologo",
                "/std:c11",
                "/W4",
                "/WX",
                "/c",
                "/I" + str(self.ffi / "include"),
                self.ffi / "tests/c/layout.c",
                "/Fo" + str(work / "layout.obj"),
            ]
        )
        self.run(
            [
                tools / "cl.exe",
                "/nologo",
                "/std:c11",
                "/W4",
                "/WX",
                "/I" + str(self.ffi / "include"),
                self.ffi / "tests/c/ownership.c",
                directory / (self.library + ".dll.lib"),
                "/Fo" + str(work / "ownership.obj"),
                "/Fe" + str(program),
            ]
        )
        load("windows").require_pe_machine(program, "x64")
        library = directory / (self.library + ".dll")
        staged = load("bindings").stage_native(library, work / library.name)
        load("windows").require_pe_machine(staged, "x64")
        subprocess.run([str(program)], cwd=work, env=self.env, check=True)
        if digest(staged) != digest(library):
            raise ValueError("The tested DLL differs from the verified build output")
        packages = ["pointer-gesture-" + self.layout.kind, self.package]
        packages.append("pointer-gesture-" + self.layout.kind + "-windows")
        self.run(
            [
                "cargo",
                "test",
                *[value for package in packages for value in ("-p", package)],
                "--locked",
                "--offline",
                "--target",
                TARGET,
            ]
        )
        (work / "result.json").write_text(
            json.dumps(
                {
                    "rid": RID,
                    **self.execution(host),
                    "nativeReceipt": digest(directory / "native-build.json"),
                    "cCaller": digest(self.ffi / "tests/c/ownership.c"),
                    "result": "passed",
                },
                indent=2,
            )
            + "\n"
        )
        print(f"Windows C ownership and Rust tests passed for {RID} on {host}.")

    def test_input(self):
        """Explicitly install hooks/inject input in an owned interactive Windows desktop."""
        host, _ = self.prepare_windows(msvc=True)
        if self.package != "pointer-gesture-recognition-ffi":
            raise ValueError("Desktop input belongs to the recognition adapter")
        directory = self.verify()
        if f"host: {HOST_TARGETS[host]}" not in self.rust().splitlines():
            raise ValueError("Use the pinned Windows Rust compiler for the host architecture")
        self.env["CARGO_TARGET_DIR"] = str(self.native / "target")
        self.run(
            [
                "cargo",
                "build",
                "-p",
                "pointer-gesture-recognition-windows",
                "--example",
                "desktop_probe",
                "--release",
                "--locked",
                "--offline",
                "--target",
                TARGET,
            ]
        )
        program = self.native / "target" / TARGET / "release/examples/desktop_probe.exe"
        load("windows").require_pe_machine(program, "x64")
        self.run([program])
        project = self.layout.tests
        load("bindings").build_managed(
            self.dotnet,
            project,
            cwd=self.root,
            env=self.env,
            properties={"UseSharedCompilation": "false"},
            verify_style=True,
        )
        output = project.parent / "bin/Release/net10.0"
        shutil.copyfile(directory / (self.library + ".dll"), output / (self.library + ".dll"))
        self.run([self.dotnet, output / (project.stem + ".dll"), "--windows-input"])

    def pack(self, version, output):
        name = self.library + ".dll"
        self.nuget.pack(version, output, {RID: (self.verify() / name, name)})

    def test_package(self, version, feed):
        host, _ = self.prepare_windows()
        self.nuget.package_consumer(
            RID,
            version,
            feed,
            execute=True,
            probe=self.native_loading_probe,
            evidence=self.execution(host),
        )

    def check_package(self, version, feed):
        self.nuget.package_consumer(RID, version, feed, execute=False)

    def native_loading_probe(self, work, package_output, env):
        """Exercise packaged managed code without NuGet's native probing metadata."""
        probe = work / "native-loading"
        probe.mkdir()
        project = ET.Element("Project", Sdk="Microsoft.NET.Sdk")
        props = ET.SubElement(project, "PropertyGroup")
        for key, value in {
            "OutputType": "Exe",
            "TargetFramework": "net10.0",
            "AnalysisLevel": "10.0-recommended",
            "UseAppHost": "false",
            "ImplicitUsings": "enable",
            "Nullable": "enable",
            "TreatWarningsAsErrors": "true",
        }.items():
            ET.SubElement(props, key).text = value
        references = ET.SubElement(project, "ItemGroup")
        for assembly in package_output.glob("NaviApps.PointerGestures.*.dll"):
            reference = ET.SubElement(references, "Reference", Include=assembly.stem)
            ET.SubElement(reference, "HintPath").text = str(assembly)
        ET.ElementTree(project).write(probe / "Probe.csproj", encoding="utf-8")
        recognition = self.id.endswith(".Recognition")
        source = """using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;
using $NAMESPACE;
if (args.Length > 0)
    NativeLibrary.SetDllImportResolver(typeof($TYPE).Assembly,
        (name, assembly, searchPath) => name == "$LIBRARY" ? NativeLibrary.Load(args[0]) : IntPtr.Zero);
try
{
    $OPERATION
    var path = Process.GetCurrentProcess().Modules.Cast<ProcessModule>()
        .Single(m => m.ModuleName.Equals("$LIBRARY.dll", StringComparison.OrdinalIgnoreCase)).FileName;
    Console.WriteLine(JsonSerializer.Serialize(new { state = "loaded", path }));
}
catch (DllNotFoundException) { Console.WriteLine("{\\"state\\":\\"missing\\"}"); }
"""
        source = (
            source.replace("$NAMESPACE", self.id)
            .replace("$LIBRARY", self.library)
            .replace("$TYPE", "GesturePatternMatcher" if recognition else "GestureGeometryPlanner")
            .replace(
                "$OPERATION",
                "using var matcher = new GesturePatternMatcher();"
                if recognition
                else "_ = GestureGeometryPlanner.PlanPattern([NaviApps.PointerGestures.Recognition.Direction.Right], 96, 64);",
            )
        )
        (probe / "Program.cs").write_text(source)
        subprocess.run(
            [
                self.dotnet,
                "restore",
                str(probe / "Probe.csproj"),
                "--configfile",
                str(work / "NuGet.Config"),
            ],
            cwd=probe,
            env=env,
            check=True,
        )
        subprocess.run(
            [
                self.dotnet,
                "build",
                str(probe / "Probe.csproj"),
                "--no-restore",
                "-c",
                "Release",
                "-p:UseSharedCompilation=false",
            ],
            cwd=probe,
            env=env,
            check=True,
        )
        output = probe / "bin/Release/net10.0"
        empty = probe / "empty"
        empty.mkdir()
        working = probe / "working"
        working.mkdir()
        name = self.library + ".dll"
        native = package_output / name
        shutil.copy2(native, working / name)
        results = {}

        def execute(case, directory, program=output, resolver=None, expected=None, environment=env):
            command = [self.dotnet, str(program / "Probe.dll")] + (
                [str(resolver)] if resolver else []
            )
            process = subprocess.run(
                command, cwd=directory, env=environment, capture_output=True, text=True, check=True
            )
            value = json.loads(process.stdout)
            if expected is None:
                if value["state"] != "missing":
                    raise ValueError("Unexpected native loading from " + case + ": " + str(value))
            elif value["state"] != "loaded" or Path(value["path"]).resolve() != expected.resolve():
                raise ValueError(
                    "Native library was not loaded from its trusted location: " + str(value)
                )
            results[case] = value

        execute("missing", empty)
        execute("working-directory-rejected", working)
        path_env = dict(env, PATH=str(working) + os.pathsep + env.get("PATH", ""))
        execute("path-rejected", empty, environment=path_env)
        trusted = probe / "assembly-directory"
        shutil.copytree(output, trusted)
        shutil.copy2(native, trusted / name)
        execute("assembly-directory", working, program=trusted, expected=trusted / name)
        custom = probe / "host-resolver"
        custom.mkdir()
        shutil.copy2(native, custom / name)
        execute("host-resolver", working, resolver=custom / name, expected=custom / name)
        print(
            "Native loading passed missing/CWD/PATH rejection, trusted assembly placement and explicit host resolution."
        )
        return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=[
            "check",
            "build",
            "verify",
            "test-native",
            "test-input",
            "pack",
            "check-package",
            "test-package",
        ],
    )
    parser.add_argument("--sdk", choices=KINDS)
    parser.add_argument("--rid", choices=(RID,), default=RID)
    add_candidate_arguments(parser)
    parser.add_argument(
        "--cross", action="store_true", help="Build with preinstalled cargo-xwin/LLVM"
    )
    parser.add_argument("--version")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--feed", type=Path)
    args = parser.parse_args()
    if args.cross and args.command != "build":
        parser.error("--cross is only valid with build")
    if args.command == "test-input" and args.sdk == "rendering":
        parser.error("Input capture belongs to recognition")
    selected = (
        (args.sdk,) if args.sdk else (("recognition",) if args.command == "test-input" else KINDS)
    )
    root = Path(__file__).resolve().parents[1]
    try:
        candidate_arguments(parser, args)
        for index, kind in enumerate(selected):
            sdk = WindowsSDK(root, kind)
            if args.command == "check":
                sdk.check(check_rust=index == 0)
            elif args.command == "build":
                sdk.build(args.cross)
            elif args.command in ("verify", "test-native", "test-input"):
                {
                    "verify": sdk.verify,
                    "test-native": sdk.test_native,
                    "test-input": sdk.test_input,
                }[args.command]()
            elif args.command == "pack":
                if not args.version or not args.output:
                    parser.error("--version and --output are required")
                sdk.pack(args.version, args.output)
            else:
                if not args.version or not args.feed:
                    parser.error("--version and --feed are required")
                {"check-package": sdk.check_package, "test-package": sdk.test_package}[
                    args.command
                ](args.version, args.feed)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + "\n")


if __name__ == "__main__":
    main()
