#!/usr/bin/env python3
"""Build host-native Rust and run the repository-owned .NET interop checks."""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

from sdk_tooling import c_headers
from sdk_tooling.adapters import load

from distribution.abi_layout import report_layout
from distribution.package_layout import DOTNET_README, KINDS, PackageLayout


def main(root=None):
    ROOT = Path(root or Path(__file__).resolve().parents[1]).resolve()
    dotnet = os.environ.get("DOTNET_BIN") or shutil.which("dotnet")
    if not dotnet:
        raise SystemExit("Install a .NET 10 SDK or set DOTNET_BIN to its executable.")
    native = ROOT / ".native-build/dotnet"
    env = dict(
        os.environ,
        RUSTUP_AUTO_INSTALL="0",
        CARGO_TARGET_DIR=str(native),
        CARGO_NET_GIT_FETCH_WITH_CLI="true",
        DOTNET_CLI_TELEMETRY_OPTOUT="1",
        DOTNET_NOLOGO="1",
        DOTNET_CLI_UI_LANGUAGE="en",
        DOTNET_CLI_USE_MSBUILD_SERVER="0",
        MSBUILDDISABLENODEREUSE="1",
        CARGO_PROFILE_RELEASE_STRIP="none",
    )
    host = subprocess.check_output(["rustc", "--version", "--verbose"], env=env, text=True)
    host = next(
        line.removeprefix("host: ") for line in host.splitlines() if line.startswith("host: ")
    )
    target = env.get("CARGO_BUILD_TARGET")
    if target and target != host:
        raise SystemExit(
            "Runtime interop checks require host-native artifacts, not a cross target."
        )
    if sys.platform != "win32":
        c_headers.prepare_generated(ROOT, env=env)
    layouts = [PackageLayout(ROOT, kind) for kind in KINDS]
    subprocess.run(
        [
            "cargo",
            "build",
            *[value for sdk in layouts for value in ("-p", sdk.package)],
            "--release",
            "--locked",
            "--offline",
        ],
        cwd=ROOT,
        env=env,
        check=True,
    )
    library_dir = native / target / "release" if target else native / "release"
    if sys.platform != "win32":
        search = "DYLD_LIBRARY_PATH" if sys.platform == "darwin" else "LD_LIBRARY_PATH"
        env[search] = str(library_dir) + os.pathsep + env.get(search, "")

    def execute(project, *arguments):
        load("dotnet").verify_managed(
            dotnet,
            project,
            cwd=project.parent,
            env=env,
            properties={"UseSharedCompilation": "false"},
        )
        subprocess.run(
            [
                dotnet,
                "build",
                project,
                "--no-restore",
                "-c",
                "Release",
                "-p:UseSharedCompilation=false",
            ],
            cwd=project.parent,
            env=env,
            check=True,
        )
        output = project.parent / "bin/Release/net10.0"
        if sys.platform == "win32":
            for sdk in layouts:
                name = sdk.package.replace("-", "_") + ".dll"
                shutil.copyfile(library_dir / name, output / name)
        subprocess.run(
            [dotnet, output / (project.stem + ".dll"), *arguments],
            cwd=project.parent,
            env=env,
            check=True,
        )

    tests = ROOT / "bindings/dotnet"
    subprocess.run(
        [
            dotnet,
            "restore",
            "--configfile",
            "NuGet.Config",
            "-p:Configuration=Release",
            "-p:UseSharedCompilation=false",
        ],
        cwd=tests,
        env=env,
        check=True,
    )
    for sdk in layouts:
        if sys.platform == "win32":
            execute(sdk.tests)
        else:
            compiler = ["xcrun", "clang"] if sys.platform == "darwin" else ["cc"]
            with tempfile.TemporaryDirectory(prefix="abi-layout-", dir=native) as work:
                layout = report_layout(sdk.ffi, Path(work), compiler=compiler, env=env)
                execute(sdk.tests, "--abi-layout", layout)
    examples = re.findall(r"```csharp\n(.*?)\n```", (ROOT / DOTNET_README).read_text(), re.S)
    if not examples:
        raise SystemExit(f"{DOTNET_README} must contain a runnable C# example.")
    for index, source in enumerate(examples, 1):
        with tempfile.TemporaryDirectory(prefix="readme-", dir=native) as directory:
            work = Path(directory)
            project = ET.Element("Project", Sdk="Microsoft.NET.Sdk")
            properties = ET.SubElement(project, "PropertyGroup")
            for name, value in {
                "OutputType": "Exe",
                "TargetFramework": "net10.0",
                "AnalysisLevel": "10.0-recommended",
                "UseAppHost": "false",
                "ImplicitUsings": "enable",
                "Nullable": "enable",
                "TreatWarningsAsErrors": "true",
            }.items():
                ET.SubElement(properties, name).text = value
            references = ET.SubElement(project, "ItemGroup")
            for sdk in layouts:
                ET.SubElement(references, "ProjectReference", Include=str(sdk.project))
            ET.ElementTree(project).write(work / "Example.csproj", encoding="utf-8")
            (work / "Program.cs").write_text(source + "\n")
            subprocess.run(
                [
                    dotnet,
                    "restore",
                    "--configfile",
                    str(tests / "NuGet.Config"),
                    "-p:Configuration=Release",
                    "-p:UseSharedCompilation=false",
                ],
                cwd=work,
                env=env,
                check=True,
            )
            execute(work / "Example.csproj")
        print("README C# example " + str(index) + " passed.")


if __name__ == "__main__":
    main()
