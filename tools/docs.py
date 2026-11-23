#!/usr/bin/env python3
"""Build Swift DocC catalogs and verify declaration help for owned public APIs."""

import argparse
from pathlib import Path

from tools._tooling import load


def undocumented_symbols(graph, source_root):
    missing = []
    for symbol in graph.get("symbols", []):
        try:
            location = Path.from_uri(symbol.get("location", {}).get("uri", ""))
        except ValueError:
            continue
        if not location.is_relative_to(source_root):
            continue
        # Extension containers and Equatable witnesses add no independent API contract.
        if symbol["kind"]["identifier"] == "swift.extension" or (
            symbol["kind"]["identifier"] == "swift.func.op"
            and symbol["names"]["title"] == "==(_:_:)"
        ):
            continue
        if not any(line["text"].strip() for line in symbol.get("docComment", {}).get("lines", [])):
            missing.append(".".join(symbol["pathComponents"]))
    return missing


def main(root=None):
    root = Path(root or Path(__file__).resolve().parent.parent).resolve()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=root / ".native-build/docs")
    args = parser.parse_args()

    def validate(module, graphs):
        missing = sorted(
            {name for graph in graphs for name in undocumented_symbols(graph, root / "Sources")}
        )
        if missing:
            raise ValueError(
                "Missing public Swift declaration documentation: " + ", ".join(missing)
            )

    load("docs").check_catalogs(
        root,
        sorted((root / "Sources").glob("*/*.docc")),
        output=args.output,
        bundle_identifier_prefix="com.naviapps.",
        graph_arguments=("--skip-synthesized-members",),
        convert_arguments=("--enable-experimental-external-link-support",),
        symbol_validator=validate,
        retain_archives=True,
    )


if __name__ == "__main__":
    main()
