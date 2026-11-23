#!/usr/bin/env python3
"""Build or verify independent recognition and rendering frameworks."""

import argparse
from pathlib import Path

from tools._tooling import load
from tools.sdk import KINDS, SDK


def builder(root, kind):
    layout = SDK(Path(root).resolve(), kind)
    return load("native").NativeBuilder(
        layout.root,
        str(layout.ffi.relative_to(layout.root)),
        symbol_prefix="pointer_gesture_recognition_"
        if kind == "recognition"
        else "pointer_gesture_rendering_",
        minimum_macos="26.0",
        receipt=layout.module + ".build.json",
        notice_packages=[SDK(layout.root, item).package for item in KINDS],
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", choices=KINDS)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    for kind in (args.sdk,) if args.sdk else KINDS:
        selected = builder(Path(__file__).resolve().parents[1], kind)
        selected.verify() if args.verify else selected.build()
