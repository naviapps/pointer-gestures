"""Build or verify a source-pinned, self-contained browser SDK package."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import tomllib
from pathlib import Path

from sdk_tooling.adapters import load

from distribution.source_identity import source_identity

VERSION = "0.2.104"
PORTABLE_CRATES = ("recognition", "rendering")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inputs(root):
    paths = [
        root / "Cargo.toml",
        root / "rust-toolchain.toml",
        root / "LICENSE",
        root / "distribution/npm_package.py",
        root / "distribution/source_identity.py",
        root / "bindings/web/package.json",
        root / "docs/WEB.md",
    ]
    crates = [root / "crates" / kind for kind in PORTABLE_CRATES]
    crates.append(root / "bindings/web/rust")
    for crate in crates:
        paths.append(crate / "Cargo.toml")
        paths.extend((crate / "src").rglob("*.rs"))
    paths.append(root / "bindings/web/rust/Cargo.lock")
    paths.extend((root / "crates/rendering/benches").rglob("*.rs"))
    paths.extend((root / "bindings/web/src").glob("*"))
    paths.extend((root / "ThirdParty/Rust").glob("*"))
    tooling = load("artifacts")
    result = tooling.hashes(paths, root)
    # Keep the receipt's original input name for the local artifact helper.
    result["@sdk-tooling/artifacts.py"] = digest(Path(tooling.__file__))
    return result


def package_hashes(directory):
    return load("artifacts").hashes(
        [
            path
            for path in directory.rglob("*")
            if path.is_file()
            and path.relative_to(directory).as_posix() not in {"provenance.json", "SHA256SUMS"}
        ],
        directory,
    )


def verify(root, directory):
    receipt = json.loads((directory / "provenance.json").read_text())
    if receipt["inputs"] != inputs(root):
        raise SystemExit("Browser SDK sources changed; rebuild its package.")
    if receipt["outputs"] != package_hashes(directory):
        raise SystemExit("Browser SDK package bytes differ from provenance.")
    expected = {**receipt["outputs"], "provenance.json": digest(directory / "provenance.json")}
    actual = dict(
        (name, checksum)
        for checksum, name in (
            line.split("  ", 1) for line in (directory / "SHA256SUMS").read_text().splitlines()
        )
    )
    if actual != expected:
        raise SystemExit("Browser SDK checksums differ from the package.")
    print("Browser SDK package matches its source snapshot and exact artifact hashes.")


def stage_workspace(root, workspace):
    """Make every compiled path dependency relative to the browser workspace root."""
    web = root / "bindings/web/rust"
    manifest = (web / "Cargo.toml").read_text()
    portable = workspace / "portable"
    portable.mkdir(parents=True)
    for kind in PORTABLE_CRATES:
        source = root / "crates" / kind
        destination = portable / kind
        shutil.copytree(source / "src", destination / "src")
        shutil.copyfile(source / "Cargo.toml", destination / "Cargo.toml")
        manifest = manifest.replace(
            f'path = "../../../crates/{kind}"', f'path = "portable/{kind}"', 1
        )
    shutil.copytree(root / "crates/rendering/benches", portable / "rendering/benches")

    # Keep the portable manifests' inherited metadata and the browser's separate
    # dependency resolution. Workspace members would pull extra optional features
    # into the browser lockfile even when the browser does not build those features.
    native = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    package = "\n".join(f"{key} = {json.dumps(value)}" for key, value in native["package"].items())
    (portable / "Cargo.toml").write_text(
        "[workspace]\n"
        + f"members = {json.dumps(PORTABLE_CRATES)}\n"
        + f"resolver = {json.dumps(native['resolver'])}\n\n"
        + "[workspace.package]\n"
        + package
        + "\n"
    )
    excluded = [f"portable/{kind}" for kind in PORTABLE_CRATES]
    manifest = manifest.replace(
        "[workspace]\n", f"[workspace]\nexclude = {json.dumps(excluded)}\n", 1
    )
    staged = tomllib.loads(manifest)
    if staged["workspace"].get("exclude") != excluded or any(
        staged["dependencies"][f"pointer-gesture-{kind}"]["path"] != f"portable/{kind}"
        for kind in PORTABLE_CRATES
    ):
        raise ValueError("Browser manifest no longer describes the expected portable crate layout.")
    (workspace / "Cargo.toml").write_text(manifest)
    shutil.copyfile(web / "Cargo.lock", workspace / "Cargo.lock")
    shutil.copytree(web / "src", workspace / "src")


def build(root, directory):
    expected_compiler = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"][
        "channel"
    ]
    compiler = subprocess.check_output(["rustc", "--version"], text=True).strip()
    if not compiler.startswith("rustc " + expected_compiler + " "):
        raise SystemExit("Use the Rust compiler pinned in rust-toolchain.toml.")
    binding = subprocess.check_output(["wasm-bindgen", "--version"], text=True).strip()
    if binding != "wasm-bindgen " + VERSION:
        raise SystemExit("Preinstall wasm-bindgen-cli " + VERSION + " matching the locked crate.")
    source_inputs = inputs(root)
    revision, fingerprint = source_identity(root)
    env = dict(os.environ)
    flags = (
        env["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
        if env.get("CARGO_ENCODED_RUSTFLAGS")
        else ([] if "CARGO_ENCODED_RUSTFLAGS" in env else env.get("RUSTFLAGS", "").split())
    )
    directory.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".web-package-", dir=directory.parent) as temporary:
        work = Path(temporary).resolve()
        workspace = work / "workspace"
        target = work / "target"
        stage_workspace(root, workspace)
        manifest = workspace / "Cargo.toml"
        env["CARGO_TARGET_DIR"] = str(target)
        cargo_home = (root / env.get("CARGO_HOME", Path.home() / ".cargo")).resolve()
        env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(
            [
                *flags,
                f"--remap-path-prefix={root}=/pointer-gestures",
                f"--remap-path-prefix={work}=/pointer-gestures-web",
                f"--remap-path-prefix={cargo_home}=/cargo-home",
            ]
        )
        subprocess.run(
            [
                "cargo",
                "build",
                "--manifest-path",
                str(manifest),
                "--release",
                "--locked",
                "--offline",
                "--features",
                "web",
                "--target",
                "wasm32-unknown-unknown",
            ],
            cwd=root,
            env=env,
            check=True,
        )
        stage = work / "package"
        subprocess.run(
            [
                "wasm-bindgen",
                str(target / "wasm32-unknown-unknown/release/pointer_gestures_wasm.wasm"),
                "--target",
                "web",
                "--out-dir",
                str(stage),
                "--out-name",
                "pointer_gestures_wasm",
            ],
            check=True,
        )
        for source in (root / "bindings/web/src").glob("*"):
            shutil.copyfile(source, stage / source.name)
        shutil.copyfile(root / "bindings/web/package.json", stage / "package.json")
        shutil.copyfile(root / "LICENSE", stage / "LICENSE")
        shutil.copyfile(root / "docs/WEB.md", stage / "README.md")
        shutil.copytree(root / "ThirdParty/Rust", stage / "THIRD_PARTY_NOTICES/Rust")
        metadata = json.loads(
            subprocess.check_output(
                [
                    "cargo",
                    "metadata",
                    "--manifest-path",
                    str(manifest),
                    "--features",
                    "web",
                    "--locked",
                    "--offline",
                    "--format-version",
                    "1",
                ],
                cwd=root,
                env=env,
                text=True,
            )
        )
        dependencies = {}
        for package in metadata["packages"]:
            if not package["source"]:
                continue
            name = package["name"] + "-" + package["version"]
            dependencies[name] = {"source": package["source"], "license": package["license"]}
            source = Path(package["manifest_path"]).parent
            destination = stage / "THIRD_PARTY_NOTICES" / name
            destination.mkdir(parents=True)
            for notice in source.iterdir():
                if notice.is_file() and notice.name.upper().startswith(
                    ("LICENSE", "COPYING", "COPYRIGHT", "NOTICE")
                ):
                    shutil.copyfile(notice, destination / notice.name)
        if source_inputs != inputs(root):
            raise SystemExit("Browser SDK inputs changed during build; output was not replaced.")
        receipt = {
            "schemaVersion": 1,
            "sourceRepository": "https://github.com/naviapps/pointer-gestures",
            "sourceRevision": revision,
            "compiler": compiler,
            "wasmBindgen": binding,
            "target": "wasm32-unknown-unknown",
            "inputs": source_inputs,
            "dependencies": dependencies,
            "outputs": package_hashes(stage),
        }
        if fingerprint is not None:
            receipt["sourceFingerprint"] = fingerprint
        (stage / "provenance.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        checksums = {**receipt["outputs"], "provenance.json": digest(stage / "provenance.json")}
        (stage / "SHA256SUMS").write_text(
            "".join(f"{checksum}  {name}\n" for name, checksum in sorted(checksums.items()))
        )
        if directory.exists():
            if (
                not (directory / "package.json").is_file()
                or json.loads((directory / "package.json").read_text())["name"]
                != "@naviapps/pointer-gestures"
            ):
                raise SystemExit("Refusing to replace an unowned browser package directory.")
        load("artifacts").publish(stage, directory, lambda: verify(root, directory))


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=root / ".web-build/package")
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    if args.verify:
        verify(root, args.output.resolve())
    else:
        build(root, args.output.resolve())


if __name__ == "__main__":
    main()
