#!/usr/bin/env python3
"""Pack verified native assets and run clean .NET package consumers."""

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
import xml.etree.ElementTree as ET
import zipfile
from contextlib import contextmanager
from pathlib import Path

from tools._tooling import load
from tools.native import builder
from tools.sdk import KINDS, SDK

MACOS_RIDS = ("osx-arm64", "osx-x64")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


@contextmanager
def package_workspace(rid):
    prefix = "pointer-package-"
    if rid.startswith("win-"):
        with load("workspaces").temporary_workspace(None, prefix=prefix) as work:
            yield work
    else:
        with tempfile.TemporaryDirectory(prefix=prefix) as temporary:
            yield Path(temporary)


class NuGetSDK:
    def __init__(self, root, kind, env, dotnet, report_dir):
        self.root = root
        self.layout = SDK(root, kind)
        self.id = self.layout.managed_id
        self.env = env
        self.dotnet = dotnet
        self.report_dir = report_dir

    def pack(self, version, output, assets):
        if not re.fullmatch(r"\d+\.\d+\.\d+-[0-9A-Za-z][0-9A-Za-z.-]*", version):
            raise ValueError("Local candidates require an explicit prerelease version")
        output = output.resolve()
        output.mkdir(parents=True, exist_ok=True)
        candidate = output / (self.id + "." + version + ".nupkg")
        if candidate.exists():
            raise ValueError("Choose a new candidate version; existing packages are immutable")
        with tempfile.TemporaryDirectory(dir=output) as temporary:
            stage = Path(temporary)
            staged_candidate = stage / candidate.name
            runtime_files = {
                f"runtimes/{rid}/native/{name}": path for rid, (path, name) in assets.items()
            }
            for member, source in runtime_files.items():
                load("bindings").stage_native(
                    source,
                    stage / "package-inputs" / self.layout.kind / member.removeprefix("runtimes/"),
                )
            load("bindings").pack_project(
                self.dotnet,
                self.layout.project,
                output=stage,
                env=self.env,
                cwd=self.root,
                nuget_config=self.root / "bindings/dotnet/NuGet.Config",
                properties={
                    "UseSharedCompilation": "false",
                    "Version": version,
                    "NativePackageRoot": str(stage / "package-inputs"),
                    "PublishRepositoryUrl": "false",
                    "EnableSourceControlManagerQueries": "false",
                    "IncludeSourceRevisionInInformationalVersion": "false",
                },
            )
            with zipfile.ZipFile(staged_candidate) as package:
                if "lib/net10.0/" + self.id + ".dll" not in package.namelist():
                    raise ValueError("Missing managed assembly")
                documentation = ET.fromstring(package.read("lib/net10.0/" + self.id + ".xml"))
                if documentation.findtext("assembly/name") != self.id or not documentation.findall(
                    "members/member"
                ):
                    raise ValueError("Missing or mismatched XML API documentation")
            notices = {
                "licenses/" + notice.relative_to(self.root / "ThirdParty").as_posix(): notice
                for notice in (self.root / "ThirdParty").rglob("*")
                if notice.is_file() and notice.name not in (".DS_Store", "registry-notices.json")
            }
            load("bindings").audit_package(
                staged_candidate,
                expected_files={
                    "README.md": self.root / "README.md",
                    "licenses/LICENSE": self.root / "LICENSE",
                    **notices,
                },
                runtime_files=runtime_files,
            )
            candidate.hardlink_to(staged_candidate)
        print(
            "Local candidate: "
            + str(candidate)
            + ". Run the execution checks for each selected runtime."
        )

    def package_consumer(self, rid, version, feed, *, execute, probe=None, evidence=None):
        # A fresh restore proves the package works without sibling projects, source builds or global caches.
        feed = feed.resolve()
        candidate = feed / (self.id + "." + version + ".nupkg")
        package_hash = digest(candidate)
        self.report_dir.mkdir(parents=True, exist_ok=True)
        with package_workspace(rid) as work:
            project = ET.Element("Project", Sdk="Microsoft.NET.Sdk")
            props = ET.SubElement(project, "PropertyGroup")
            for key, value in {
                "OutputType": "Exe",
                "TargetFramework": "net10.0",
                "AnalysisLevel": "10.0-recommended",
                "RuntimeIdentifier": rid,
                "UseAppHost": "false",
                "SelfContained": "false",
                "ImplicitUsings": "enable",
                "Nullable": "enable",
                "TreatWarningsAsErrors": "true",
            }.items():
                ET.SubElement(props, key).text = value
            ET.SubElement(
                ET.SubElement(project, "ItemGroup"),
                "PackageReference",
                Include=self.id,
                Version=version,
            )
            ET.ElementTree(project).write(work / "Consumer.csproj", encoding="utf-8")
            shutil.copyfile(self.layout.tests.parent / "Program.cs", work / "Program.cs")
            config = ET.Element("configuration")
            feeds = ET.SubElement(config, "packageSources")
            ET.SubElement(feeds, "clear")
            ET.SubElement(feeds, "add", key="local", value=str(feed))
            ET.ElementTree(config).write(work / "NuGet.Config", encoding="utf-8")
            env = self.env.copy()
            env["NUGET_PACKAGES"] = str(work / "packages")
            if rid.startswith("osx-"):
                env = {key: value for key, value in env.items() if not key.startswith("DYLD_")}
            env["PATH"] = os.pathsep.join(
                p for p in env.get("PATH", "").split(os.pathsep) if ".native-build" not in p
            )
            subprocess.run(
                [
                    self.dotnet,
                    "restore",
                    str(work / "Consumer.csproj"),
                    "--configfile",
                    str(work / "NuGet.Config"),
                ],
                cwd=work,
                env=env,
                check=True,
            )
            subprocess.run(
                [
                    self.dotnet,
                    "build",
                    "--no-restore",
                    "-c",
                    "Release",
                    "-p:UseSharedCompilation=false",
                ],
                cwd=work,
                env=env,
                check=True,
            )
            restored = {}
            assets = json.loads((work / "obj/project.assets.json").read_text())
            output = work / "bin/Release/net10.0" / rid
            expected = {self.id: self.layout}
            if self.layout.kind == "rendering":
                recognition = SDK(self.root, "recognition")
                expected[recognition.managed_id] = recognition
            for identity, library in assets["libraries"].items():
                if library["type"] != "package":
                    raise ValueError("Consumer resolved a source project instead of a package")
                name, resolved_version = identity.rsplit("/", 1)
                if name not in expected:
                    raise ValueError("Unexpected package in the dependency closure: " + name)
                if resolved_version != version:
                    raise ValueError("Consumer resolved a different package version: " + identity)
                package = feed / (name + "." + resolved_version + ".nupkg")
                installed = Path(env["NUGET_PACKAGES"]) / library["path"]
                cached = installed / (name.lower() + "." + resolved_version + ".nupkg")
                if digest(cached) != digest(package):
                    raise ValueError(
                        "Installed package differs from the local candidate: " + identity
                    )
                restored[identity] = digest(package)
                with zipfile.ZipFile(package) as archive:
                    documentation = "lib/net10.0/" + name + ".xml"
                    if (installed / documentation).read_bytes() != archive.read(documentation):
                        raise ValueError("Installed API documentation differs from the package")
                    library_name = expected[name].package.replace("-", "_")
                    filename = (
                        f"lib{library_name}.dylib"
                        if rid.startswith("osx-")
                        else library_name + ".dll"
                    )
                    member = f"runtimes/{rid}/native/{filename}"
                    library = output / filename
                    if library.read_bytes() != archive.read(member):
                        raise ValueError("Selected native asset differs from the package")
                    if rid.startswith("win-"):
                        from tools.windows import MACHINE, inspect_pe, public_exports

                        windows_members = [
                            item.filename
                            for item in archive.infolist()
                            if item.filename.startswith("runtimes/win") and not item.is_dir()
                        ]
                        if windows_members != [member]:
                            raise ValueError(
                                "Windows packages require exactly one x64 native library"
                            )
                        inspect_pe(library, MACHINE, public_exports(expected[name]))
            if {identity.rsplit("/", 1)[0] for identity in restored} != expected.keys():
                raise ValueError(
                    "Installed package dependency closure differs from the SDK contract"
                )
            loading = None
            if execute:
                subprocess.run(
                    [self.dotnet, str(output / "Consumer.dll")], cwd=work, env=env, check=True
                )
                if probe:
                    loading = probe(work, output, env)
        if digest(candidate) != package_hash:
            raise ValueError("Package changed during validation")
        receipt = "package-check-" if execute else "package-build-check-"
        (self.report_dir / (receipt + rid + ".json")).write_text(
            json.dumps(
                {
                    "rid": rid,
                    "package": candidate.name,
                    "sha256": package_hash,
                    "restoredPackages": restored,
                    "testSource": digest(self.layout.tests.parent / "Program.cs"),
                    ("windowsExecuted" if rid.startswith("win-") else "macOSExecuted"): execute,
                    "nativeLoading": loading,
                    **(evidence or {}),
                    "result": "passed",
                },
                indent=2,
            )
            + "\n"
        )
        print(
            "Clean package "
            + ("execution" if execute else "restore/build and asset selection")
            + " passed for "
            + rid
            + "."
            + (
                " Input hooks and drawing remain host-adapter checks."
                if execute
                else " No native code was executed."
            )
        )


def macos_asset(root, kind):
    native = builder(root, kind)
    native.verify(inspect_native=sys.platform == "darwin")
    library = (
        native.artifact / "macos-arm64_x86_64" / (native.module + ".framework") / native.module
    )
    return library, f"lib{native.library}.dylib"


def require_macos_host(rid, dotnet, env):
    architecture = "arm64" if rid == "osx-arm64" else "x86_64"
    if sys.platform != "darwin" or platform.machine() != architecture:
        raise ValueError("Run package tests on the matching native macOS architecture")
    translated = subprocess.run(
        ["sysctl", "-in", "sysctl.proc_translated"], text=True, capture_output=True
    )
    if translated.stdout.strip() == "1":
        raise ValueError("Rosetta is separate from native package execution")
    info = subprocess.check_output([dotnet, "--info"], env=env, text=True)
    if not re.search(r"Architecture:\s+" + rid.removeprefix("osx-") + r"\b", info):
        raise ValueError("Select the target-native .NET SDK using DOTNET_BIN")


def main():
    from tools.windows import RID, WindowsSDK

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("pack", "check-package", "test-package"))
    parser.add_argument("--sdk", choices=KINDS)
    parser.add_argument("--rid", action="append", choices=(*MACOS_RIDS, RID), required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--feed", type=Path)
    args = parser.parse_args()
    rids = tuple(dict.fromkeys(args.rid))
    if args.command == "pack":
        if not args.output or args.feed:
            parser.error("Packing requires --output")
    elif len(rids) != 1 or not args.feed or args.output:
        parser.error("Checking a package requires one --rid and --feed")
    if not re.fullmatch(r"\d+\.\d+\.\d+-[0-9A-Za-z][0-9A-Za-z.-]*", args.version):
        parser.error("Local candidates require an explicit prerelease version")
    root = Path(__file__).resolve().parent.parent
    env = dict(
        os.environ,
        DOTNET_CLI_TELEMETRY_OPTOUT="1",
        DOTNET_NOLOGO="1",
        DOTNET_CLI_UI_LANGUAGE="en",
        DOTNET_CLI_USE_MSBUILD_SERVER="0",
        MSBUILDDISABLENODEREUSE="1",
    )
    dotnet = env.get("DOTNET_BIN") or shutil.which("dotnet") or "dotnet"
    try:
        if args.command == "test-package" and rids[0] in MACOS_RIDS:
            require_macos_host(rids[0], dotnet, env)
        for kind in (args.sdk,) if args.sdk else KINDS:
            if args.command == "pack":
                assets = {}
                if any(rid in MACOS_RIDS for rid in rids):
                    asset = macos_asset(root, kind)
                    assets.update((rid, asset) for rid in rids if rid in MACOS_RIDS)
                if RID in rids:
                    sdk = WindowsSDK(root, kind)
                    name = sdk.library + ".dll"
                    assets[RID] = (sdk.verify() / name, name)
                NuGetSDK(root, kind, env, dotnet, None).pack(args.version, args.output, assets)
            elif rids[0] == RID:
                sdk = WindowsSDK(root, kind)
                if args.command == "test-package":
                    sdk.test_package(args.version, args.feed)
                else:
                    sdk.check_package(args.version, args.feed)
            else:
                sdk = NuGetSDK(root, kind, env, dotnet, root / ".native-build/macos" / kind)
                sdk.package_consumer(
                    rids[0], args.version, args.feed, execute=args.command == "test-package"
                )
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + "\n")


if __name__ == "__main__":
    main()
