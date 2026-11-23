"""Run an independent installed macOS C client for each SDK."""

import argparse
from pathlib import Path

from sdk_tooling.adapters import load
from sdk_tooling.native_plan import builder

from distribution.package_layout import KINDS

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
        native = builder(ROOT, kind)
        framework = native.artifact / "macos-arm64_x86_64" / (native.module + ".framework")
        argv = ["--language", "c"]
        if args.revision is not None:
            argv.extend(("--revision", args.revision))
        if args.evidence_dir is not None:
            argv.extend(("--evidence", str(args.evidence_dir / f"pointer-c-{kind}-consumer.json")))
        check_installed(
            root=ROOT,
            git_url="https://github.com/naviapps/pointer-gestures",
            rust_packages=(),
            framework=framework.relative_to(ROOT).as_posix(),
            header=f"pointer_gesture_{kind}.h",
            c_fixture=f"fixtures/consumers/c/{kind}.c",
            c_receipt=native.receipt.relative_to(ROOT).as_posix(),
            argv=argv,
        )


if __name__ == "__main__":
    main()
