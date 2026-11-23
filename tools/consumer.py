"""Build independent Swift consumers from only the installable Swift sources/artifacts."""

import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[1]
    products = (
        "PointerGestureRecognition",
        "PointerGestureRecognitionMacOS",
        "PointerGestureRendering",
        "PointerGestureRenderingSwiftUI",
        "PointerGestureRenderingMacOS",
    )
    with tempfile.TemporaryDirectory(prefix="pointer-consumer-") as directory:
        work = Path(directory)
        sdk = work / "pointer-gestures"
        sdk.mkdir()
        shutil.copyfile(root / "Package.swift", sdk / "Package.swift")
        for name in ("Sources", "Tests", "Artifacts"):
            shutil.copytree(root / name, sdk / name, symlinks=True)
        consumer = work / "consumer"
        consumer.mkdir()
        targets = []
        for index, product in enumerate(products):
            name = "Consumer" + str(index)
            source = consumer / "Sources" / name
            source.mkdir(parents=True)
            usage = ""
            if product == "PointerGestureRecognition":
                types = (
                    "GesturePoint",
                    "GestureDirection",
                    "PointerButton",
                    "GestureModifierFlags",
                    "GestureEventDisposition",
                    "GestureInputEvent",
                    "GestureInputEvent.Kind",
                    "GestureReplayRequest",
                    "GestureRecognitionContext",
                    "GestureRecognizerTuning",
                    "GestureRecognizerTuning.ValidationError",
                    "GestureRecognizerState",
                    "GestureRecognizerState.Status",
                    "GestureRecognizerState.Trace",
                    "GestureSessionReport",
                    "GestureRecognizerState.Status.Lifecycle",
                    "GestureRecognizerFailure",
                    "GesturePatternMatcher<Int>",
                    "GestureRecognizerConfiguration<Int>",
                    "GestureObservationToken",
                    "GestureRecognizer<Int>",
                )
                usage = "func requireSendable<Value: Sendable>(_: Value.Type) {}\n"
                usage += "".join("requireSendable(" + item + ".self)\n" for item in types)
                usage += "func requireHashable<Value: Hashable>(_: Value.Type) {}\n"
                usage += "requireHashable(GestureDirection.self)\n"
                usage += "func requireEquatable<Value: Equatable>(_: Value.Type) {}\n"
                usage += "requireEquatable(GestureEventDisposition.self)\n"
                usage += "requireEquatable(GestureRecognizerTuning.self)\n"
                usage += "requireEquatable(GestureRecognizerState.Status.self)\n"
                usage += "requireEquatable(GestureRecognizerState.Trace.self)\n"
                usage += "requireEquatable(GestureSessionReport.self)\n"
            if product == "PointerGestureRendering":
                usage = "func requireSendable<Value: Sendable>(_: Value.Type) {}\n"
                usage += "requireSendable(GestureOverlayTrace.self)\n"
                usage += "requireSendable(GestureGeometryError.self)\n"
                usage += "func requireEquatable<Value: Equatable>(_: Value.Type) {}\n"
                usage += "requireEquatable(GestureOverlayTrace.self)\n"
                usage += "requireEquatable(GesturePatternGeometry.self)\n"
                usage += (
                    "precondition(GesturePath.planPattern([], canvasSize: .zero) == .failure(.empty))\n"
                    "let path = try GesturePath.planLiveTrace(\n"
                    "  directions: [.right], points: [.zero, .init(x: 40, y: 0)]).get()\n"
                    "precondition(path.commands.last == .line(to: .init(x: 40, y: 0)))\n"
                    "let converter = GestureOverlayTraceConverter()\n"
                    "let trace = converter.convert(.init(isVisible: true, directions: [.right],\n"
                    "  directionEndpoints: [.zero, .init(x: 40, y: 0)]))\n"
                    "precondition(converter.convert(.init()) == nil)\n"
                    "precondition(trace?.points.count == 2)\n"
                    "let first = GestureOverlayState(nowNanoseconds: 0)\n"
                    "let second = GestureOverlayState(nowNanoseconds: 0)\n"
                    "precondition(first.update(trace, nowNanoseconds: 0)\n"
                    "  == second.update(trace, nowNanoseconds: 0))\n"
                )
            if product == "PointerGestureRenderingSwiftUI":
                usage = "func requireSendable<Value: Sendable>(_: Value.Type) {}\n"
                usage += "".join(
                    "requireSendable(" + item + ".self)\n"
                    for item in (
                        "GestureOverlayCoordinateMapping",
                        "GestureOverlayConfiguration",
                        "GesturePatternConfiguration",
                        "GestureOverlayModel",
                        "GestureOverlayView",
                        "GesturePatternView",
                    )
                )
                usage += "func requireEquatable<Value: Equatable>(_: Value.Type) {}\n"
                usage += "".join(
                    "requireEquatable(" + item + ".self)\n"
                    for item in (
                        "GestureOverlayCoordinateMapping",
                        "GestureOverlayConfiguration",
                        "GesturePatternConfiguration",
                    )
                )
            if product == "PointerGestureRenderingMacOS":
                usage = (
                    "let coordinator = GestureOverlayCoordinator(\n"
                    "  configurationProvider: { _ in .liveOverlay }, screensProvider: { [] })\n"
                    "coordinator.removeOverlays()\n"
                )
            (source / "main.swift").write_text(
                "import " + product + "\n" + usage + 'print("' + product + ' imported")\n'
            )
            targets.append(
                '.executableTarget(name: "'
                + name
                + '", dependencies: [.product(name: "'
                + product
                + '", package: "pointer-gestures")])'
            )
        (consumer / "Package.swift").write_text(
            "// swift-tools-version: 6.3\nimport PackageDescription\n"
            'let package = Package(name: "Consumers", platforms: [.macOS(.v26)], '
            'dependencies: [.package(path: "../pointer-gestures")], targets: ['
            + ",".join(targets)
            + "])\n"
        )
        subprocess.run(
            [
                "swift",
                "build",
                "-Xswiftc",
                "-strict-concurrency=complete",
                "-Xswiftc",
                "-warnings-as-errors",
            ],
            cwd=consumer,
            check=True,
        )
        binary = Path(
            subprocess.check_output(
                ["swift", "build", "--show-bin-path"], cwd=consumer, text=True
            ).strip()
        )
        for index, product in enumerate(products):
            subprocess.run([str(binary / ("Consumer" + str(index)))], cwd=consumer, check=True)
        print(
            json.dumps(
                {
                    "result": "passed",
                    "products": products,
                    "sourceBuildInputs": ["Package.swift", "Sources", "Tests", "Artifacts"],
                }
            )
        )


if __name__ == "__main__":
    main()
