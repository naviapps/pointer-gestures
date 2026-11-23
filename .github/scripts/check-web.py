"""Run the Web SDK gate using prepared runtimes and lease-local compiler inputs."""

import base64
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.request
from pathlib import Path, PurePosixPath

STD = "rust-std-1.99.1-wasm32-unknown-unknown"
BINDGEN = "wasm-bindgen-0.2.104-aarch64-apple-darwin"
ARCHIVES = {
    STD + ".tar.xz": (
        "https://static.rust-lang.org/dist/2026-10-15/" + STD + ".tar.xz",
        "5f628f857dd00a8aa3fcac2e275665d2782066a5795274ed124c5b560f46423c",
        23180912,
    ),
    BINDGEN + ".tar.gz": (
        "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/0.2.104/"
        + BINDGEN
        + ".tar.gz",
        "167c0dd3366b42c3475d8b2f3c97472fecdf98ced4c234ffe6a86ec741f7dad8",
        6784484,
    ),
}


def require(value, message):
    if not value:
        raise RuntimeError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        require(path.resolve(strict=True).is_relative_to(root), "Prepared Rust link leaves prefix")
        if path.is_file():
            result[path.relative_to(root).as_posix()] = digest(path)
    return result


def extract(archive, destination, root):
    destination.mkdir()
    seen, total = set(), 0
    with tarfile.open(archive) as stream:
        for member in stream:
            name = member.name.rstrip("/")
            path = PurePosixPath(name)
            require(
                name
                and path.as_posix() == name
                and not path.is_absolute()
                and ".." not in path.parts
                and "\\" not in name
                and "\0" not in name
                and path.parts[0] == root
                and name not in seen
                and member.sparse is None
                and (member.isdir() or member.type == tarfile.REGTYPE),
                "Unsafe compiler archive member",
            )
            seen.add(name)
            target = destination / name
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            total += member.size
            require(
                0 <= member.size and total <= 256 * 1024 * 1024 and len(seen) < 20000,
                "Compiler archive exceeds extraction bounds",
            )
            target.parent.mkdir(parents=True, exist_ok=True)
            with stream.extractfile(member) as source, target.open("xb") as output:
                shutil.copyfileobj(source, output)
            require(target.stat().st_size == member.size, "Compiler archive truncated")
            target.chmod(0o700 if member.mode & 0o111 else 0o600)


def run(command, root, env, log=None):
    print("Running " + " ".join(map(str, command)), flush=True)
    if log is None:
        return subprocess.check_output(command, cwd=root, env=env, text=True).strip()
    with log.open("x") as output:
        subprocess.run(
            command,
            cwd=root,
            env=env,
            stdout=output,
            stderr=subprocess.STDOUT,
            check=True,
            timeout=900,
        )


def main():
    root = Path(__file__).resolve().parents[2]
    tools = Path(os.environ["NAVI_CI_TOOLS_ROOT"])
    require(tools.is_absolute(), "Prepared tools root must be absolute")
    tools = tools.resolve(strict=True)
    temporary = Path(os.environ["RUNNER_TEMP"])
    artifacts = Path(os.environ["NAVI_CI_ARTIFACTS"])
    require(
        temporary.is_absolute() and artifacts.is_absolute(), "Expected lease-local destinations"
    )
    artifacts.mkdir(mode=0o700, exist_ok=True)
    output = artifacts / "web"
    output.mkdir()
    node, npm, python = (
        tools / path for path in ("node/bin/node", "node/bin/npm", "python/bin/python3")
    )
    require(
        python.resolve(strict=True) == Path(sys.executable).resolve(strict=True),
        "Expected the prepared Python executable",
    )
    for path in (node, npm, python):
        require(
            path.resolve(strict=True).is_relative_to(tools) and os.access(path, os.X_OK),
            "Prepared runtime leaves its prefix",
        )
    home = {key: os.environ.get(key) for key in ("HOME", "CFFIXED_USER_HOME")}
    runtimes = {str(path): digest(path) for path in (node, npm, python)}
    prepared = inventory(tools / "rust")
    provider = {key: os.environ.get(key) for key in ("GIT_CONFIG_GLOBAL", "NAVI_CI_SOURCES")}
    provider_bytes = {key: digest(Path(value)) for key, value in provider.items() if value}
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("RUST", "CARGO", "DYLD_", "NPM_", "npm_", "NODE_"))
        and key not in {"NAVI_CI_SOURCE_COMMIT", "NAVI_CI_SOURCE_FINGERPRINT"}
    }
    env.update(
        PATH=f"{node.parent}:{python.parent}:/usr/bin:/bin:/usr/sbin:/sbin",
        NPM_BIN=str(npm),
        PYTHONDONTWRITEBYTECODE="1",
        RUSTUP_AUTO_INSTALL="0",
    )
    revision = run(["/usr/bin/git", "rev-parse", "HEAD"], root, env)
    require(revision == os.environ["GITHUB_SHA"], "Checkout differs from workflow source")
    env["NAVI_CI_SOURCE_COMMIT"] = revision
    versions = {
        "node": run([node, "--version"], root, env),
        "npm": run([npm, "--version"], root, env),
        "python": run([python, "--version"], root, env),
    }
    require(
        versions == {"node": "v24.22.1", "npm": "11.19.1", "python": "Python 3.15.1"},
        "Prepared runtime versions differ",
    )
    summary = {
        "sourceRevision": revision,
        "versions": versions,
        "result": "failed",
        "typeScriptCompilerRun": False,
        "browserRun": False,
    }
    type_pin = json.loads((root / "tools/typescript-dist.json").read_text())
    require(
        type_pin["package"] == "typescript"
        and type_pin["version"]
        == tomllib.loads((root / "mise.toml").read_text())["tools"]["npm:typescript"]
        and type_pin["url"]
        == "https://registry.npmjs.org/typescript/-/typescript-" + type_pin["version"] + ".tgz",
        "TypeScript distribution differs from the declared developer compiler",
    )
    try:
        with tempfile.TemporaryDirectory(prefix="pointer-web-", dir=temporary) as directory:
            work = Path(directory)
            type_archive = work / "typescript.tgz"
            with urllib.request.urlopen(type_pin["url"], timeout=120) as response:
                type_bytes = response.read(type_pin["bytes"] + 1)
            require(
                len(type_bytes) == type_pin["bytes"]
                and hashlib.sha256(type_bytes).hexdigest() == type_pin["sha256"]
                and "sha512-" + base64.b64encode(hashlib.sha512(type_bytes).digest()).decode()
                == type_pin["integrity"],
                "Pinned TypeScript archive checksum, integrity or size differs",
            )
            summary["typeScriptDistribution"] = dict(type_pin, verified=True, leaseLocal=True)
            type_archive.write_bytes(type_bytes)
            extract(type_archive, work / "typescript", "package")
            tsc = work / "typescript/package/bin/tsc"
            require(
                run([node, tsc, "--version"], root, env) == "Version " + type_pin["version"],
                "Pinned TypeScript compiler version differs",
            )
            env.update(
                TSC_BIN=str(tsc),
                POINTER_GESTURES_TYPESCRIPT_TRACE=str(output / "typescript-resolution.log"),
            )
            local_rust = work / "rust"
            shutil.copytree(tools / "rust", local_rust, symlinks=False)
            for name, (url, checksum, size) in ARCHIVES.items():
                archive = work / name
                with (
                    urllib.request.urlopen(url, timeout=120) as response,
                    archive.open("xb") as target,
                ):
                    raw = response.read(size + 1)
                    require(
                        len(raw) == size and hashlib.sha256(raw).hexdigest() == checksum,
                        "Pinned compiler archive checksum or size differs",
                    )
                    target.write(raw)
                extract(
                    archive, work / (name + "-extracted"), STD if name.startswith(STD) else BINDGEN
                )
            component = (
                work
                / (STD + ".tar.xz-extracted")
                / STD
                / "rust-std-wasm32-unknown-unknown/lib/rustlib/wasm32-unknown-unknown"
            )
            destination = local_rust / "lib/rustlib/wasm32-unknown-unknown"
            if destination.exists():
                require(
                    inventory(destination) == inventory(component), "Prepared Wasm target differs"
                )
            else:
                shutil.copytree(component, destination)
            bindgen = work / (BINDGEN + ".tar.gz-extracted") / BINDGEN / "wasm-bindgen"
            require(
                digest(bindgen)
                == "205b32c819381930451d14c09800795698c605ffdcebc9a827310e7cf459087e",
                "Pinned wasm-bindgen executable differs",
            )
            env.update(
                PATH=f"{local_rust}/bin:{bindgen.parent}:{env['PATH']}",
                CARGO_HOME=str(work / "cargo-home"),
                RUSTUP_HOME=str(work / "unused-rustup"),
                RUSTC=str(local_rust / "bin/rustc"),
                RUSTDOC=str(local_rust / "bin/rustdoc"),
            )
            require(
                run([local_rust / "bin/rustc", "--version"], root, env)
                == "rustc 1.99.1 (58cf8f9c3 2026-10-13)",
                "Prepared Rust version differs",
            )
            require(
                run([bindgen, "--version"], root, env) == "wasm-bindgen 0.2.104",
                "Pinned wasm-bindgen version differs",
            )
            lock = tomllib.loads((root / "bindings/web/rust/Cargo.lock").read_text())
            require(
                all(
                    entry["source"] == "registry+https://github.com/rust-lang/crates.io-index"
                    and len(entry["checksum"]) == 64
                    for entry in lock["package"]
                    if "source" in entry
                ),
                "Web dependencies must use the locked public registry",
            )
            cargo = local_rust / "bin/cargo"
            run(
                [cargo, "fetch", "--manifest-path", "bindings/web/rust/Cargo.toml", "--locked"],
                root,
                env,
                output / "cargo-fetch.log",
            )
            env["CARGO_NET_OFFLINE"] = "true"
            for source in sorted(root.rglob("*.mjs")):
                if ".web-build" not in source.parts:
                    run([node, "--check", source], root, env)
            run(["/usr/bin/make", "check-web"], root, env, output / "web-check.log")
            run([python, "-B", "-m", "tools.web", "--verify"], root, env, output / "web-verify.log")
            consumers = [
                json.loads(line)
                for line in (output / "web-check.log").read_text().splitlines()
                if line.startswith('{"result":"passed",')
            ]
            require(
                len(consumers) == 1 and consumers[0]["sourceRevision"] == revision,
                "Exact-source installed npm consumer did not pass",
            )
            (output / "web-consumer.json").write_text(json.dumps(consumers[0], indent=2) + "\n")
            shutil.copytree(root / ".web-build/package", output / "package")
            summary.update(
                result="passed",
                consumer=consumers[0],
                packageProvenanceSHA256=digest(output / "package/provenance.json"),
            )
    finally:
        check_log = output / "web-check.log"
        type_results = (
            [
                json.loads(line.removeprefix("POINTER_TYPESCRIPT_ACCEPTANCE "))
                for line in check_log.read_text().splitlines()
                if line.startswith("POINTER_TYPESCRIPT_ACCEPTANCE ")
            ]
            if check_log.exists()
            else []
        )
        require(len(type_results) <= 1, "Multiple TypeScript compiler results were reported")
        if summary["result"] == "passed":
            require(
                len(type_results) == 1 and summary["consumer"]["typeScript"] == type_results[0],
                "Successful Web gate requires the installed-package TypeScript result",
            )
        if type_results:
            require(
                type_results[0]["version"] == type_pin["version"]
                and type_results[0]["compileExitCode"] == 0
                and type_results[0]["compilerVersion"] == "Version " + type_pin["version"]
                and type_results[0]["installedPackage"] is True
                and type_results[0]["compileOnly"] is True
                and type_results[0]["noEmit"] is True
                and type_results[0]["packageDeclarationResolved"] is True
                and type_results[0]["expectedMisuseCases"] == 7
                and type_results[0]["resolutionTraceSHA256"]
                == digest(output / "typescript-resolution.log"),
                "TypeScript compiler result differs",
            )
            summary["typeScriptCompilerRun"] = True
            summary["typeScript"] = type_results[0]
            (output / "typescript-consumer.json").write_text(
                json.dumps(type_results[0], indent=2) + "\n"
            )
        require(
            home == {key: os.environ.get(key) for key in home}
            and home == {key: env.get(key) for key in home}
            and provider == {key: env.get(key) for key in provider}
            and provider_bytes
            == {key: digest(Path(value)) for key, value in provider.items() if value}
            and runtimes == {str(path): digest(path) for path in (node, npm, python)}
            and prepared == inventory(tools / "rust"),
            "Prepared tools or inherited settings changed",
        )
        summary["preparedToolsAndInheritedSettingsUnchanged"] = True
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
