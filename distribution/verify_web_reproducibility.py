"""Compare a committed Web SDK build across independent source paths."""

import os
import subprocess
import tarfile
import tempfile
from pathlib import Path

from sdk_tooling import vm_environment as vm
from sdk_tooling.adapters import python_command

from distribution.npm_package import digest


def check(root, revision):
    env = dict(os.environ, GIT_NO_LAZY_FETCH="1")
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
            artifact = source / ".web-build/package"
            return {
                path.relative_to(artifact).as_posix(): digest(path)
                for path in artifact.rglob("*")
                if path.is_file()
            }

        first = build_at(scratch / "sdk")
        second = build_at(scratch / "nested path with spaces" / "pointer-gestures")
        if first != second:
            different = sorted(
                name for name in first.keys() | second.keys() if first.get(name) != second.get(name)
            )
            raise SystemExit("Web SDK files differ across source paths: " + ", ".join(different))
        wasm = first["pointer_gestures_wasm_bg.wasm"]
        print(f"Web SDK {revision} reproduced all {len(first)} package files. Wasm SHA-256 {wasm}")


def main():
    root = Path(__file__).resolve().parents[1]
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    check(root, revision)


if __name__ == "__main__":
    main()
