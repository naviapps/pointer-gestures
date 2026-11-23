SDK_TOOLING ?= mise run sdk-tooling --
BROWSER_ARTIFACTS ?= $(shell mktemp -d)
SWIFT_STRICT_FLAGS := -Xswiftc -strict-concurrency=complete -Xswiftc -warnings-as-errors
ARTIFACTS := $(CURDIR)/Artifacts

.DEFAULT_GOAL := check
.PHONY: check check-source test-managed test-rust check-distribution test-distribution-tools check-docs check-native test-consumer check-rust-consumer check-c-consumer native test-ffi check-dotnet check-windows check-web check-web-source check-web-types check-web-reproducibility check-web-browser build build-release clean format lint test

check: check-source test test-consumer

check-source: lint test-rust test-ffi test-managed

check-distribution: test-distribution-tools check-native check-docs build-release

test-consumer:
	$(SDK_TOOLING) swift-consumer

check-rust-consumer:
	$(SDK_TOOLING) python -m distribution.rust_consumer

check-c-consumer:
	$(SDK_TOOLING) python -m distribution.c_consumer

test-distribution-tools:
	$(SDK_TOOLING) python -m unittest discover -s distribution/tests

check-docs:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
	$(SDK_TOOLING) docs

check-native:
	$(SDK_TOOLING) xcframework --verify

test-rust:
	cargo test --workspace --locked --offline
	cargo clippy --workspace --all-targets --locked --offline -- -D warnings

native:
	$(SDK_TOOLING) xcframework

# The maintained C callers use ASan; the prebuilt Rust frameworks do not.
test-ffi:
	mkdir -p .native-build
	@set -e; for kind in recognition rendering; do \
	  if [ "$$kind" = recognition ]; then module=CPointerGestureRecognition; else module=CPointerGestureRendering; fi; \
	  framework="$(ARTIFACTS)/$$module.xcframework/macos-arm64_x86_64"; \
	  xcrun clang -std=c11 -Wall -Wextra -Werror -fsyntax-only -Icrates/$$kind-ffi/include crates/$$kind-ffi/tests/c/layout.c; \
	  xcrun clang -std=c11 -Wall -Wextra -Werror -fsanitize=address -Icrates/$$kind-ffi/include crates/$$kind-ffi/tests/c/ownership.c -F"$$framework" -framework "$$module" -Xlinker -rpath -Xlinker "$$framework" -o .native-build/$$kind-ffi-ownership; \
	  .native-build/$$kind-ffi-ownership; \
	done

check-dotnet:
	$(SDK_TOOLING) python -m distribution.verify_dotnet

check-windows:
	$(SDK_TOOLING) python -m distribution.windows_sdk check

# Web tools and dependencies remain independent of native consumer validation.
check-web: check-web-source
	$(SDK_TOOLING) python -m distribution.npm_package
	node --test bindings/web/tests/*.test.mjs
	node distribution/verify_web_consumer.mjs

check-web-source:
	cargo test --manifest-path bindings/web/rust/Cargo.toml --locked --offline
	cargo clippy --manifest-path bindings/web/rust/Cargo.toml --all-targets --features web --locked --offline -- -D warnings

check-web-types:
	node distribution/verify_web_consumer.mjs --types-only

check-web-reproducibility:
	$(SDK_TOOLING) python -m distribution.verify_web_reproducibility

check-web-browser:
	$(SDK_TOOLING) python -m distribution.verify_web_browser --artifacts "$(BROWSER_ARTIFACTS)"

build:
	swift build $(SWIFT_STRICT_FLAGS)

build-release:
	swift build -c release $(SWIFT_STRICT_FLAGS)

clean:
	$(SDK_TOOLING) clean .build target .native-build fixtures/consumers/swift/.build \
	  bindings/web/rust/target .web-build fixtures/acceptance/browser/node_modules \
	  bindings/dotnet/examples/windows/bin bindings/dotnet/examples/windows/obj \
	  bindings/dotnet/src/NaviApps.PointerGestures.Recognition/bin \
	  bindings/dotnet/src/NaviApps.PointerGestures.Recognition/obj \
	  bindings/dotnet/src/NaviApps.PointerGestures.Rendering/bin \
	  bindings/dotnet/src/NaviApps.PointerGestures.Rendering/obj \
	  bindings/dotnet/tests/NaviApps.PointerGestures.Recognition.Tests/bin \
	  bindings/dotnet/tests/NaviApps.PointerGestures.Recognition.Tests/obj \
	  bindings/dotnet/tests/NaviApps.PointerGestures.Rendering.Tests/bin \
	  bindings/dotnet/tests/NaviApps.PointerGestures.Rendering.Tests/obj

format:
	$(SDK_TOOLING) quality format --dotnet

lint:
	$(SDK_TOOLING) quality check --dotnet

test:
	swift test $(SWIFT_STRICT_FLAGS)

test-managed:
	dotnet build --configuration Release bindings/dotnet/tests/NaviApps.PointerGestures.Recognition.Tests/NaviApps.PointerGestures.Recognition.Tests.csproj
	dotnet build --configuration Release bindings/dotnet/tests/NaviApps.PointerGestures.Rendering.Tests/NaviApps.PointerGestures.Rendering.Tests.csproj
