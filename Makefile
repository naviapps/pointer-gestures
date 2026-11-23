SWIFT_STRICT_FLAGS := -Xswiftc -strict-concurrency=complete -Xswiftc -warnings-as-errors
ARTIFACTS := $(CURDIR)/Artifacts

.DEFAULT_GOAL := check
.PHONY: check check-distribution check-tools check-docs check-native check-consumer check-rust-consumer check-c-consumer native test-ffi check-dotnet check-windows check-web check-web-types check-web-reproducibility check-web-browser build build-release format lint test

check: check-distribution test check-consumer

check-distribution: lint check-tools check-native check-docs test-ffi build-release

check-consumer:
	python3 -m tools.consumer

check-rust-consumer:
	python3 -B -m tools.rust_consumer

check-c-consumer:
	python3 -B -m tools.c_consumer

check-tools:
	python3 -m unittest discover -s tools/tests

check-docs:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
	python3 -m tools.docs

check-native:
	python3 -m tools.native --verify
	cargo test --workspace --locked --offline
	cargo clippy --workspace --all-targets --locked --offline -- -D warnings

native:
	python3 -m tools.native

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
	python3 -m tools.dotnet

check-windows:
	python3 -m tools.windows check

# Web tools and dependencies remain independent of native consumer validation.
check-web:
	cargo test --manifest-path bindings/web/rust/Cargo.toml --locked --offline
	cargo clippy --manifest-path bindings/web/rust/Cargo.toml --all-targets --features web --locked --offline -- -D warnings
	python3 -m tools.web
	node --test bindings/web/tests/*.test.mjs
	node tools/web-consumer.mjs

check-web-types:
	node tools/web-consumer.mjs --types-only

check-web-reproducibility:
	python3 -E -S -B -m tools.web_reproducibility

# Uses the package staged by CI under NAVI_CI_ARTIFACTS/web/package.
# Set POINTER_GESTURES_WEB_PACKAGE to select a verified package in the owned guest.
check-web-browser:
	python3 -I -B .github/scripts/check-web-browser.py

build:
	swift build $(SWIFT_STRICT_FLAGS)

build-release:
	swift build -c release $(SWIFT_STRICT_FLAGS)

format:
	mise exec -- python3 -B tools/quality.py format --dotnet

lint:
	mise exec -- python3 -B tools/quality.py check --dotnet

test:
	swift test $(SWIFT_STRICT_FLAGS)
