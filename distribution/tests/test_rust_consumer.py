import unittest

from distribution.rust_consumer import RECOGNITION, RENDERING, verify_normal_graph


def edge(name, kind=None):
    return {"pkg": name, "dep_kinds": [{"kind": kind}]}


def graph(client_dependencies, rendering_dependencies=(), raster=False):
    ffi = "pointer-gesture-recognition-ffi"
    dependencies = {
        "client": client_dependencies,
        RECOGNITION: [],
        RENDERING: [edge(RECOGNITION), *rendering_dependencies],
        ffi: [edge("tiny-skia")],
        "tiny-skia": [],
    }
    return {
        "packages": [{"id": name, "name": name} for name in dependencies],
        "resolve": {
            "root": "client",
            "nodes": [
                {
                    "id": name,
                    "deps": deps,
                    "features": ["raster"] if raster and name == RENDERING else [],
                }
                for name, deps in dependencies.items()
            ],
        },
    }


class RustConsumerGraphTests(unittest.TestCase):
    def test_unselected_and_non_normal_edges_do_not_leak_into_recognition(self):
        metadata = graph(
            [
                edge(RECOGNITION),
                edge(RENDERING, "dev"),
                edge("pointer-gesture-recognition-ffi", "build"),
            ]
        )
        self.assertEqual(
            verify_normal_graph(metadata, "recognition")["normalDependencies"], [RECOGNITION]
        )

    def test_unexpected_raster_default_rejects_geometry(self):
        metadata = graph([edge(RECOGNITION), edge(RENDERING)], [edge("tiny-skia")], raster=True)
        with self.assertRaisesRegex(ValueError, "only by the raster client"):
            verify_normal_graph(metadata, "geometry")
        self.assertEqual(
            verify_normal_graph(metadata, "raster")["normalDependencies"],
            [RECOGNITION, RENDERING, "tiny-skia"],
        )

    def test_linked_ffi_crate_rejects_portable_consumer(self):
        metadata = graph([edge(RECOGNITION), edge("pointer-gesture-recognition-ffi")])
        with self.assertRaisesRegex(ValueError, "FFI or Windows crate"):
            verify_normal_graph(metadata, "recognition")
