"""Build the paired Recognition and Rendering NuGet packages for distribution."""

import argparse
from pathlib import Path

from sdk_tooling import c_headers, nuget
from sdk_tooling.xcframework import xcframework

from distribution.nuget import MACOS_RIDS
from distribution.package_layout import DOTNET_README, KINDS
from distribution.windows_sdk import RID, WindowsSDK

ROOT = Path(__file__).resolve().parents[1]
RUNTIMES = (*MACOS_RIDS, RID)


def pack(root, runtimes):
    c_headers.prepare_generated(root)
    package_files = {
        "README.md": root / DOTNET_README,
        "LICENSE": root / "LICENSE",
        **nuget.retained_notice_files(root / "ThirdParty", "licenses"),
    }

    packages = []
    for kind in KINDS:
        sdk = WindowsSDK(root, kind)
        packages.append(
            nuget.pack_native(
                xcframework(root, kind),
                project=sdk.csproj,
                package_files=package_files,
                nuget_config=root / "bindings/dotnet/NuGet.Config",
                runtimes=runtimes,
                sources=[packages[0].parent] if packages else [],
                input_files=[
                    root / "bindings/dotnet/src/Shared/SnapshotValues.cs",
                    root / "distribution/pack_nuget.py",
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
