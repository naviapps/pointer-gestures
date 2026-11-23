"""Declaration help must cover owned APIs without counting imported symbols."""

import tempfile
import unittest
from pathlib import Path

from sdk_tooling.adapters import load

documentation = load("sdk_checks")


class DeclarationDocumentationTests(unittest.TestCase):
    def test_missing_and_blank_owned_help_fail_in_paths_with_spaces(self):
        with tempfile.TemporaryDirectory() as directory:
            sources = Path(directory) / "SDK Sources"
            symbol = {
                "location": {"uri": (sources / "State.swift").as_uri()},
                "kind": {"identifier": "swift.method"},
                "names": {"title": "update(_:now:)"},
                "pathComponents": ["State", "update(_:now:)"],
            }
            for comment in [{}, {"docComment": {"lines": [{"text": "   "}]}}]:
                with (
                    self.subTest(comment=comment),
                    self.assertRaisesRegex(ValueError, r"State\.update\(_:now:\)"),
                ):
                    documentation._validate_declaration_docs(
                        sources, [{"symbols": [dict(symbol, **comment)]}]
                    )
            symbol["docComment"] = {
                "lines": [{"text": "Submit state at absolute monotonic nanoseconds."}]
            }
            documentation._validate_declaration_docs(sources, [{"symbols": [symbol]}])

    def test_imported_help_and_standard_witnesses_are_not_owned_contracts(self):
        def symbol(path, kind="swift.method", title="run()"):
            return {
                "location": {"uri": path.as_uri()},
                "kind": {"identifier": kind},
                "names": {"title": title},
                "pathComponents": ["Example", title],
            }

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sources = root / "sdk/Sources"
            value = sources / "Value.swift"
            graph = {
                "symbols": [
                    symbol(root / "dependency/Sources/Other.swift"),
                    symbol(root / "sdk/SourcesElsewhere/Other.swift"),
                    symbol(value, "swift.extension"),
                    symbol(value, "swift.func.op", "==(_:_:)"),
                    {"kind": {"identifier": "swift.method"}, "pathComponents": ["Imported"]},
                ]
            }
            documentation._validate_declaration_docs(sources, [graph])


if __name__ == "__main__":
    unittest.main()
