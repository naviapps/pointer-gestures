"""Provision and run bounded browser acceptance on macOS ARM64."""

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
from pathlib import Path, PurePosixPath


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run(command, root, env, log, timeout):
    with log.open("x") as output:
        subprocess.run(
            command,
            cwd=root,
            env=env,
            stdout=output,
            stderr=subprocess.STDOUT,
            check=True,
            timeout=timeout,
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifacts",
        type=Path,
        required=True,
        help="Write evidence under this absolute path outside the repository",
    )
    parser.add_argument(
        "--package", type=Path, help="Generated package (default: .web-build/package)"
    )
    parser.add_argument("--source-revision", help="Commit of a previously verified SDK package")
    parser.add_argument("--provenance-sha256", help="Expected SHA-256 of the package provenance")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    fixture = root / "fixtures/acceptance/browser"
    require(
        platform.system() == "Darwin" and platform.machine() == "arm64",
        "Browser acceptance requires a macOS ARM64 runner",
    )
    temporary = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir())
    artifacts = args.artifacts
    require(temporary.is_absolute() and artifacts.is_absolute(), "Expected absolute paths")
    temporary, artifacts = temporary.resolve(), artifacts.resolve()
    require(
        not temporary.is_relative_to(root) and not artifacts.is_relative_to(root),
        "Expected temporary and artifact paths outside source",
    )
    package = (args.package or root / ".web-build/package").resolve(strict=True)
    output = artifacts / "web-browser"
    require(
        not temporary.is_relative_to(package) and not output.is_relative_to(package),
        "Expected temporary and artifact paths outside the SDK package",
    )
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    require(source == os.environ.get("GITHUB_SHA", source), "Checkout differs from Actions source")
    expected = args.source_revision or source
    require(
        re.fullmatch("[a-f0-9]{40}", source) and re.fullmatch("[a-f0-9]{40}", expected),
        "Expected immutable fixture and SDK source identities",
    )
    provenance_sha = digest(package / "provenance.json")
    if expected != source:
        require(args.provenance_sha256, "A reused SDK package needs its pinned provenance digest")
    if args.provenance_sha256:
        require(provenance_sha == args.provenance_sha256, "SDK package provenance differs")
    provenance = json.loads((package / "provenance.json").read_text())
    require(provenance["sourceRevision"] == expected, "SDK package source differs")
    hashes = {"provenance.json": provenance_sha, **provenance["outputs"]}
    for name, checksum in hashes.items():
        path = PurePosixPath(name)
        require(
            path.as_posix() == name and not path.is_absolute() and ".." not in path.parts,
            "Unsafe package output path",
        )
        target = (package / name).resolve(strict=True)
        require(target.is_relative_to(package) and digest(target) == checksum, "SDK bytes differ")
    checksums = dict(
        (name, checksum)
        for checksum, name in (
            line.split("  ", 1) for line in (package / "SHA256SUMS").read_text().splitlines()
        )
    )
    require(checksums == hashes, "SDK package checksums differ")

    node_path, npm_path = shutil.which("node"), shutil.which("npm")
    require(node_path and npm_path, "Run with the Node and npm versions selected by mise")
    node, npm, python = Path(node_path), Path(npm_path), Path(sys.executable)
    output.mkdir(parents=True)
    before = {str(path): digest(path) for path in (node, npm, python)}
    summary = {
        "result": "failed",
        "fixtureSourceRevision": source,
        "sdkSourceRevision": expected,
        "packageProvenanceSHA256": provenance_sha,
        "browserRun": False,
    }
    try:
        with tempfile.TemporaryDirectory(prefix="pointer-browser-", dir=temporary) as directory:
            work = Path(directory)
            tooling = work / "tooling"
            tooling.mkdir()
            for name in ("package.json", "package-lock.json"):
                shutil.copyfile(fixture / name, tooling / name)
            env = {
                key: value
                for key, value in os.environ.items()
                if not key.startswith(("NPM_", "npm_", "NODE_", "PLAYWRIGHT_", "DYLD_"))
            }
            for name in ("npmrc", "global-npmrc"):
                (work / name).touch()
            env.update(
                PATH=f"{node.parent}:/usr/bin:/bin:/usr/sbin:/sbin",
                NPM_BIN=str(npm),
                TMPDIR=str(work),
                npm_config_userconfig=str(work / "npmrc"),
                npm_config_globalconfig=str(work / "global-npmrc"),
                npm_config_cache=str(work / "npm-cache"),
                npm_config_registry="https://registry.npmjs.org/",
                npm_config_fetch_retries="1",
                npm_config_fetch_timeout="60000",
                PLAYWRIGHT_BROWSERS_PATH=str(work / "browsers"),
                PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD="1",
            )
            versions = {
                name: subprocess.check_output([path, "--version"], env=env, text=True).strip()
                for name, path in (("node", node), ("npm", npm))
            }
            require(
                versions == {"node": "v24.22.1", "npm": "11.19.1"},
                "Expected the pinned Node and npm versions",
            )
            summary["versions"] = versions
            run(
                [npm, "ci", "--ignore-scripts", "--no-audit", "--no-fund"],
                tooling,
                env,
                output / "npm-ci.log",
                180,
            )
            specification = json.loads((tooling / "package.json").read_text())
            installed = tooling / "node_modules"
            for name in ("playwright", "playwright-core"):
                require(
                    json.loads((installed / name / "package.json").read_text())["version"]
                    == specification["devDependencies"]["playwright"],
                    "Installed Playwright version differs",
                )
            chromium = next(
                item
                for item in json.loads((installed / "playwright-core/browsers.json").read_text())[
                    "browsers"
                ]
                if item["name"] == "chromium"
            )
            require(
                chromium["revision"] == specification["browserGate"]["chromiumRevision"]
                and chromium["browserVersion"] == specification["browserGate"]["chromiumVersion"],
                "Pinned Chromium descriptor differs",
            )
            cli = installed / "playwright/cli.js"
            del env["PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD"]
            run(
                [node, cli, "install", "--no-shell", "chromium"],
                tooling,
                env,
                output / "chromium-install.log",
                300,
            )
            summary["browserRun"] = True
            run(
                [
                    node,
                    root / "distribution/run_browser_acceptance.mjs",
                    "--package",
                    package,
                    "--work",
                    work,
                    "--artifacts",
                    output,
                    "--sdk-source-revision",
                    expected,
                    "--fixture-source-revision",
                    source,
                ],
                root,
                env,
                output / "browser.log",
                180,
            )
            receipt = json.loads((output / "browser-consumer.json").read_text())
            require(
                receipt["result"] == "passed"
                and receipt["fixtureSourceRevision"] == source
                and receipt["sdkSourceRevision"] == expected
                and receipt["packageProvenanceSHA256"] == provenance_sha
                and len(receipt["views"]) == 2,
                "Installed SDK browser acceptance did not pass",
            )
            for view in receipt["views"]:
                require(
                    view["result"] == "passed" and all(view["checks"].values()),
                    "Browser check failed",
                )
                require(len(view["screenshots"]) == 3, "Browser screenshot evidence missing")
                for screenshot in view["screenshots"]:
                    require(
                        digest(output / screenshot["path"]) == screenshot["sha256"],
                        "Screenshot differs",
                    )
                require(
                    (output / (view["name"] + "-trace.zip")).stat().st_size > 0,
                    "Browser trace missing",
                )
            summary.update(
                result="passed", browserReceiptSHA256=digest(output / "browser-consumer.json")
            )
    finally:
        unchanged = before == {str(path): digest(path) for path in (node, npm, python)}
        summary["runtimesUnchanged"] = unchanged
        if not unchanged:
            summary["result"] = "failed"
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        require(unchanged, "Browser runtime executables changed")


if __name__ == "__main__":
    main()
