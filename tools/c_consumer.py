"""Run an independent installed macOS C client for each SDK."""

import argparse
from pathlib import Path

from tools._tooling import load
from tools.sdk import KINDS, SDK

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", help="Exact tested SDK commit; defaults to clean HEAD")
    parser.add_argument(
        "--evidence-dir", type=Path, help="Retain one JSON report per SDK outside the checkout"
    )
    args = parser.parse_args()

    check_installed = load("installed_consumers").check_installed
    for kind in KINDS:
        module = SDK(ROOT, kind).module
        argv = ["--language", "c"]
        if args.revision is not None:
            argv.extend(("--revision", args.revision))
        if args.evidence_dir is not None:
            argv.extend(("--evidence", str(args.evidence_dir / f"pointer-c-{kind}-consumer.json")))
        check_installed(
            root=ROOT,
            git_url="https://github.com/naviapps/pointer-gestures",
            rust_packages=(),
            framework=f"Artifacts/{module}.xcframework/macos-arm64_x86_64/{module}.framework",
            header=f"pointer_gesture_{kind}.h",
            c_fixture=f"fixtures/c-consumer/{kind}.c",
            c_receipt=f"Artifacts/{module}.build.json",
            argv=argv,
        )


if __name__ == "__main__":
    main()
