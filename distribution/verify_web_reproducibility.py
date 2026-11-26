"""Compare a committed Web SDK build across independent source paths."""

import argparse
import os
import subprocess
import tarfile
import tempfile
from pathlib import Path

from sdk_tooling import vm_environment as vm
from sdk_tooling.adapters import python_command

from distribution.npm_package import digest


def package_file_hashes(package):
    return {
        path.relative_to(package).as_posix(): digest(path)
        for path in package.rglob("*")
        if path.is_file()
    }


def check(root, revision, *, with_consumers=False, artifacts=None):
    env = dict(os.environ, GIT_NO_LAZY_FETCH="1")
    # The tooling launcher does not forward Make's jobserver descriptors.
    for name in ("CARGO_MAKEFLAGS", "MAKEFLAGS", "MFLAGS"):
        env.pop(name, None)
    vm.set_value(env, vm.Variable.SOURCE_COMMIT, revision)
    vm.set_value(env, vm.Variable.SOURCE_FINGERPRINT, None)
    with tempfile.TemporaryDirectory(prefix="pointer-web-reproducibility-") as temporary:
        scratch = Path(temporary)
        archive = scratch / "source.tar"
        with archive.open("wb") as output:
            subprocess.run(
                ["git", "archive", "--format=tar", revision],
                cwd=root,
                env=env,
                stdout=output,
                check=True,
            )

        def build_at(source):
            with tarfile.open(archive) as snapshot:
                snapshot.extractall(source, filter="data")
            subprocess.run(
                python_command(source, "-m", "distribution.npm_package"),
                cwd=source,
                env=env,
                check=True,
            )
            return source / ".web-build/package"

        source = scratch / "sdk"
        package = build_at(source)
        first = package_file_hashes(package)
        second = package_file_hashes(
            build_at(scratch / "nested path with spaces" / "pointer-gestures")
        )
        if first != second:
            different = sorted(
                name for name in first.keys() | second.keys() if first.get(name) != second.get(name)
            )
            raise SystemExit("Web SDK files differ across source paths: " + ", ".join(different))
        wasm = first["pointer_gestures_wasm_bg.wasm"]
        print(
            f"Web SDK {revision} reproduced all {len(first)} package files. Wasm SHA-256 {wasm}",
            flush=True,
        )
        if with_consumers:
            subprocess.run(
                ["node", "distribution/verify_web_consumer.mjs", "--binding-tests", package],
                cwd=source,
                check=True,
            )
            browser_sources = [
                "distribution/verify_web_browser.py",
                "distribution/run_browser_acceptance.mjs",
                "fixtures/acceptance/browser",
            ]
            if subprocess.run(
                ["git", "diff", "--quiet", revision, "--", *browser_sources], cwd=root
            ).returncode:
                raise SystemExit("Browser fixture sources differ from the committed Web SDK check.")
            subprocess.run(
                python_command(
                    root,
                    "-m",
                    "distribution.verify_web_browser",
                    "--package",
                    package,
                    "--provenance-sha256",
                    first["provenance.json"],
                    *(["--artifacts", artifacts] if artifacts is not None else []),
                ),
                cwd=root,
                check=True,
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--with-consumers", action="store_true", help="Test the matched package")
    parser.add_argument(
        "--artifacts", type=Path, help="Browser evidence path outside the repository"
    )
    args = parser.parse_args()
    if args.artifacts is not None and not args.with_consumers:
        parser.error("--artifacts requires --with-consumers")
    root = Path(__file__).resolve().parents[1]
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    check(root, revision, with_consumers=args.with_consumers, artifacts=args.artifacts)


if __name__ == "__main__":
    main()
