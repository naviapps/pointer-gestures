"""Shared layout for the two independently consumable gesture SDKs."""

import tomllib
from dataclasses import dataclass
from pathlib import Path

KINDS = ("recognition", "rendering")
DOTNET_README = "bindings/dotnet/README.md"


@dataclass(frozen=True)
class PackageLayout:
    root: Path
    kind: str

    def __post_init__(self):
        if self.kind not in KINDS:
            raise ValueError("Unknown gesture SDK: " + self.kind)

    @property
    def ffi(self):
        return self.root / "crates" / (self.kind + "-ffi")

    @property
    def package(self):
        return tomllib.loads((self.ffi / "Cargo.toml").read_text())["package"]["name"]

    @property
    def managed_id(self):
        return "NaviApps.PointerGestures." + self.kind.title()

    @property
    def project(self):
        return self.root / "bindings/dotnet/src" / self.managed_id / (self.managed_id + ".csproj")

    @property
    def tests(self):
        name = self.managed_id + ".Tests"
        return self.root / "bindings/dotnet/tests" / name / (name + ".csproj")

    @property
    def nuget_consumer(self):
        return self.root / "fixtures/consumers/dotnet" / (self.kind.title() + ".cs")
