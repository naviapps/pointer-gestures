"""Run the SDK's three ordinary Cargo Git consumer feature selections."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
URL = "https://github.com/naviapps/pointer-gestures"
RECOGNITION = "pointer-gesture-recognition"
RENDERING = "pointer-gesture-rendering"
RUST_MODES = {
    "recognition": ((), (RECOGNITION,)),
    "geometry": (("geometry",), (RECOGNITION, RENDERING)),
    "raster": (("raster",), (RECOGNITION, RENDERING)),
}


def verify_normal_graph(metadata, mode):
    """Check the active normal graph, excluding lockfile/dev/build package unions."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    client = metadata["resolve"]["root"]
    selected = set()
    pending = [client]
    while pending:
        package = pending.pop()
        if package in selected:
            continue
        selected.add(package)
        pending.extend(
            edge["pkg"]
            for edge in nodes[package]["deps"]
            if any(kind["kind"] is None for kind in edge["dep_kinds"])
        )
    dependencies = [packages[package] for package in selected - {client}]
    names = sorted(package["name"] for package in dependencies)
    public = [name for name in names if name in {RECOGNITION, RENDERING}]
    if public != list(RUST_MODES[mode][1]):
        raise ValueError("The selected normal graph differs from the client feature selection")
    if any(
        name.startswith("pointer-gesture-") and name.endswith(("-ffi", "-windows"))
        for name in names
    ):
        raise ValueError("The portable Cargo client selected an FFI or Windows crate")
    rendering = next((package for package in dependencies if package["name"] == RENDERING), None)
    raster = rendering is not None and "raster" in nodes[rendering["id"]]["features"]
    if raster != (mode == "raster") or ("tiny-skia" in names) != raster:
        raise ValueError("Raster code and tiny-skia must be selected only by the raster client")
    return {"normalDependencies": names}


def main():
    from sdk_tooling.adapters import load

    load("consumers.rust_c").check_installed(
        root=ROOT,
        git_url=URL,
        rust_packages=(RECOGNITION, RENDERING),
        rust_modes=RUST_MODES,
        rust_mode_check=verify_normal_graph,
    )


if __name__ == "__main__":
    main()
