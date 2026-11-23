// swift-tools-version: 6.4
import PackageDescription

let package = Package(
  name: "PointerGestures",
  platforms: [.macOS(.v26)],
  products: [
    .library(name: "PointerGestureRecognition", targets: ["PointerGestureRecognition"]),
    .library(
      name: "PointerGestureRecognitionMacOS",
      targets: ["PointerGestureRecognitionMacOS"]),
    .library(name: "PointerGestureRendering", targets: ["PointerGestureRendering"]),
    .library(name: "PointerGestureRenderingSwiftUI", targets: ["PointerGestureRenderingSwiftUI"]),
    .library(name: "PointerGestureRenderingMacOS", targets: ["PointerGestureRenderingMacOS"]),
  ],
  targets: [
    .binaryTarget(
      name: "CPointerGestureRecognition", path: "Artifacts/CPointerGestureRecognition.xcframework"),
    .binaryTarget(
      name: "CPointerGestureRendering", path: "Artifacts/CPointerGestureRendering.xcframework"),
    .target(name: "PointerGestureRecognition", dependencies: ["CPointerGestureRecognition"]),
    .target(
      name: "PointerGestureRecognitionMacOS", dependencies: ["PointerGestureRecognition"]),
    .target(
      name: "PointerGestureRendering",
      dependencies: ["CPointerGestureRendering", "PointerGestureRecognition"]),
    .target(
      name: "PointerGestureRenderingSwiftUI",
      dependencies: ["PointerGestureRendering", "PointerGestureRecognition"]),
    .target(
      name: "PointerGestureRenderingMacOS",
      dependencies: [
        "PointerGestureRendering", "PointerGestureRenderingSwiftUI", "PointerGestureRecognition",
      ]),
    .testTarget(
      name: "PointerGestureRecognitionTests", dependencies: ["PointerGestureRecognition"]),
    .testTarget(
      name: "PointerGestureRecognitionMacOSTests",
      dependencies: ["PointerGestureRecognition", "PointerGestureRecognitionMacOS"]),
    .testTarget(name: "PointerGestureRenderingTests", dependencies: ["PointerGestureRendering"]),
    .testTarget(
      name: "PointerGestureRenderingMacOSTests",
      dependencies: [
        "PointerGestureRenderingMacOS", "PointerGestureRendering",
        "PointerGestureRenderingSwiftUI", "PointerGestureRecognition",
      ]),
    .testTarget(
      name: "PointerGestureRenderingSwiftUITests",
      dependencies: [
        "PointerGestureRendering", "PointerGestureRenderingSwiftUI", "PointerGestureRecognition",
      ]),
  ]
)
