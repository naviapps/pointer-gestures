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
        help="Write evidence outside the repository and test the local .web-build/package",
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    fixture = root / "bindings/web/tests/browser"
    require(
        platform.system() == "Darwin" and platform.machine() == "arm64",
        "Browser acceptance requires a macOS ARM64 runner",
    )
    if args.artifacts is None:
        tools = Path(os.environ["NAVI_CI_TOOLS_ROOT"]).resolve(strict=True)
        temporary = Path(os.environ["RUNNER_TEMP"])
        artifacts = Path(os.environ["NAVI_CI_ARTIFACTS"])
        package = Path(os.environ.get("POINTER_GESTURES_WEB_PACKAGE", artifacts / "web/package"))
        source = os.environ.get("NAVI_CI_SOURCE_COMMIT", os.environ.get("GITHUB_SHA", ""))
        expected = os.environ.get("POINTER_GESTURES_WEB_EXPECTED_SOURCE", source)
        pinned_provenance = os.environ.get("POINTER_GESTURES_WEB_EXPECTED_PROVENANCE_SHA256")
        node, npm, python = (
            tools / path for path in ("node/bin/node", "node/bin/npm", "python/bin/python3")
        )
        require(
            python.resolve(strict=True) == Path(sys.executable).resolve(strict=True),
            "Expected the prepared Python executable",
        )
        for path in (node, npm, python):
            require(
                path.resolve(strict=True).is_relative_to(tools), "Runtime leaves prepared prefix"
            )
    else:
        temporary = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir())
        artifacts = args.artifacts
        package = root / ".web-build/package"
        source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
        require(
            source == os.environ.get("GITHUB_SHA", source), "Checkout differs from Actions source"
        )
        expected = source
        pinned_provenance = None
        node_path, npm_path = shutil.which("node"), shutil.which("npm")
        require(node_path and npm_path, "Run with the Node and npm versions selected by mise")
        node, npm, python = Path(node_path), Path(npm_path), Path(sys.executable)

    require(
        temporary.is_absolute()
        and artifacts.is_absolute()
        and not temporary.resolve().is_relative_to(root)
        and not artifacts.resolve().is_relative_to(root),
        "Expected absolute temporary and artifact paths outside source",
    )
    output = artifacts / "web-browser"
    output.mkdir(parents=True)
    package = package.resolve(strict=True)
    require(
        re.fullmatch("[a-f0-9]{40}", source) and re.fullmatch("[a-f0-9]{40}", expected),
        "Expected immutable fixture and SDK source identities",
    )
    provenance_sha = digest(package / "provenance.json")
    if expected != source:
        require(pinned_provenance, "A reused SDK package needs its pinned provenance digest")
    if pinned_provenance:
        require(provenance_sha == pinned_provenance, "SDK package provenance differs")
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
    before = {str(path): digest(path) for path in (node, npm, python)}
    inherited = {name: os.environ.get(name) for name in ("HOME", "CFFIXED_USER_HOME")}
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
                POINTER_GESTURES_WEB_PACKAGE=str(package),
                POINTER_GESTURES_WEB_EXPECTED_SOURCE=expected,
                POINTER_GESTURES_BROWSER_FIXTURE_SOURCE=source,
                POINTER_GESTURES_BROWSER_WORK=str(work),
                POINTER_GESTURES_BROWSER_ARTIFACTS=str(output),
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
                [node, root / "tools/web-browser-consumer.mjs"],
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
        unchanged = before == {
            str(path): digest(path) for path in (node, npm, python)
        } and inherited == {name: os.environ.get(name) for name in inherited}
        summary["preparedRuntimesAndInheritedHomesUnchanged"] = unchanged
        if not unchanged:
            summary["result"] = "failed"
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        require(unchanged, "Prepared runtimes or inherited homes changed")


if __name__ == "__main__":
    main()
