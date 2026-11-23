# SDK development

This repository owns independent recognition and rendering libraries, their native
adapters, artifact policy and acceptance fixtures. SDK Tooling owns shared build
and verification mechanisms. Hosts own permissions onboarding, application commands
and persistence. The optional macOS and Windows rendering adapters own overlay
resources; hosts explicitly activate them and supply appearance policy.
Read README.md and the affected Swift guide in docs/ before API changes.

Use Git on main. Keep unfinished edits in the working tree; publish coherent,
verified main units only. Preserve consumer pins. Rewrite history only for an
authorized publication. Use GitHub-hosted CI without private dependency credentials.

Keep Rust recognition, Windows input and the two C ABI crates independent. Safe
recognition consumers must not emit C exports or require rendering/SwiftUI.
Rust owns state, matching, timing, recovery, geometry and presentation cadence.
Native adapters own resources and language scheduling. Coordinates increase down;
platform/view transforms stay at adapter or host boundaries.

Preserve exact matching, duplicate replacement, captured session/context snapshots,
consumed-input replay, ordered effects and final trace presentation before hiding.
Keep observation tokens, start retries and teardown lifetimes explicit. Windows
input probes require explicit opt-in inside an owned isolated desktop. Never change
global sharing, permissions, TCC, credentials or real user data for a test.

C views borrow their owners. Swift/.NET copy before release or host callbacks.
Raw core-handle destruction only releases memory; hosts deliver close recovery
before release. Native input adapters own recovery of consumed input when stopping or
destroying; explicit stop makes failures observable. Keep independent
dynamic native binaries with exactly their declared exports and retained runtime notices.
Rebuild affected frameworks and provenance after native/input changes. Swift consumers
need neither Rust nor Python. Maintain SDK APIs, guides and consumers together.
Preserve dated release evidence.

Run `make check` for lint, Rust and Swift tests, standalone Swift consumers, C ABI
fixtures and managed compilation. Use the distribution setup in README.md for
receipt, Rust documentation, .NET package and Web checks. Use injected effects and
owned native fixtures. Compilation, local tests, actual OS effects and exact-source
CI are separate evidence. Do not repeat unchanged successful checks or add tests that
freeze private helper spelling.

## Test ownership

Rust is the authority for platform-neutral recognition, matching, recovery,
geometry, bounds and presentation timing. Put full boundary/parameter matrices
there. C tests cover ABI layout, malformed buffers, borrowed ownership and exports.
Swift/.NET/Web tests cover type/error/effect translation, generic command and
snapshot lifetimes, disposal and host scheduling/reentrancy; do not repeat each
algorithm matrix in every language. Check Swift protocol conformances in the
standalone consumer build rather than many no-op runtime tests. Native fixtures
cover actual macOS/Windows input and rendering resources. Language test counts
need not match. Move missing behavior coverage before deleting a duplicate; keep
binding-specific and native regressions in their owning layer. A minimized shared
regression vector is useful only when it exercises an actual binding contract.

Run the Rust suite first for common behavior, then the affected adapter/consumer
checks. Finish changed native resource paths with an owned OS fixture. GitHub-hosted Windows CI builds and tests x64. GitHub-hosted macOS checks use
the tracked frameworks. Keep input and permission probes in separately owned
desktops. Test-only
changes need no repeated native UI run when the runtime and fixtures are byte-identical.
