"""Build the paired Recognition and Rendering NuGet packages for distribution."""

import argparse
from pathlib import Path

from sdk_tooling.adapters import load
from sdk_tooling.xcframework import xcframework

from distribution.nuget import MACOS_RIDS
from distribution.package_layout import KINDS
from distribution.windows_sdk import RID, WindowsSDK

ROOT = Path(__file__).resolve().parents[1]
RUNTIMES = (*MACOS_RIDS, RID)


def pack(root, runtimes):
    notices = root / "ThirdParty"
    license_file = root / "LICENSE"
    package_files = {"README.md": root / "README.md", "LICENSE": license_file}
    package_files.update(
        {
            "licenses/" + path.relative_to(notices).as_posix(): path
            for path in load("artifacts").files(notices)
            if path.name not in (".DS_Store", "registry-notices.json")
        }
    )
    if "licenses/LICENSE" in package_files:
        raise ValueError("Third-party notice collides with the SDK license")
    package_files["licenses/LICENSE"] = license_file

    packages = []
    for kind in KINDS:
        sdk = WindowsSDK(root, kind)
        packages.append(
            load("nuget").pack_native(
                xcframework(root, kind),
                project=sdk.csproj,
                package_files=package_files,
                nuget_config=root / "bindings/dotnet/NuGet.Config",
                runtimes=runtimes,
                sources=[packages[0].parent] if packages else [],
                input_files=[
                    root / name
                    for name in (
                        "bindings/dotnet/Directory.Build.props",
                        "bindings/dotnet/Directory.Build.targets",
                        "bindings/dotnet/src/Shared/SnapshotValues.cs",
                        "distribution/pack_nuget.py",
                    )
                ],
                windows_output_root=sdk.native,
                windows_source_inputs=sdk.sources,
            )
        )
    recognition, rendering = packages
    return rendering, recognition


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--runtime", action="append", choices=RUNTIMES, required=True)
    parser.add_argument("--result", type=Path, required=True)
    args = parser.parse_args()
    if sorted(args.runtime) != sorted(RUNTIMES):
        parser.error("The paired distribution requires each supported runtime exactly once")
    packages = pack(ROOT, args.runtime)
    with args.result.open("x", encoding="utf-8", newline="\n") as result:
        result.writelines(package.name + "\n" for package in packages)


if __name__ == "__main__":
    main()
