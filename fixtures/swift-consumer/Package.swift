// swift-tools-version: 6.3
import PackageDescription

let package = Package(
  name: "PointerGestureConsumers",
  platforms: [.macOS(.v26)],
  dependencies: [.package(name: "pointer-gestures", path: "../sdk")],
  targets: [
    .executableTarget(
      name: "RecognitionConsumer",
      dependencies: [
        .product(name: "PointerGestureRecognition", package: "pointer-gestures")
      ]),
    .executableTarget(
      name: "RecognitionMacOSConsumer",
      dependencies: [
        .product(name: "PointerGestureRecognitionMacOS", package: "pointer-gestures"),
        .product(name: "PointerGestureRecognition", package: "pointer-gestures"),
      ]),
    .executableTarget(
      name: "RenderingConsumer",
      dependencies: [
        .product(name: "PointerGestureRendering", package: "pointer-gestures")
      ]),
    .executableTarget(
      name: "RenderingSwiftUIConsumer",
      dependencies: [
        .product(name: "PointerGestureRenderingSwiftUI", package: "pointer-gestures"),
        .product(name: "PointerGestureRendering", package: "pointer-gestures"),
      ]),
    .executableTarget(
      name: "RenderingMacOSConsumer",
      dependencies: [
        .product(name: "PointerGestureRenderingMacOS", package: "pointer-gestures")
      ]),
  ]
)
