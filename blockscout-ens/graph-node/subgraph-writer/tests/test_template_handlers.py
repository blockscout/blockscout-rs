"""Every event handler declared in the subgraph template manifest must be
exported from one of the template's mapping sources.

graph-cli rejects a subgraph with "Missing handlers in WASM: <name>" when a
handler listed in subgraph.yaml is not exported by the compiled mapping.
"""

import re
import unittest
from pathlib import Path

TEMPLATE_DIR = (
    Path(__file__).resolve().parent.parent
    / "templater"
    / "{{cookiecutter.short_name}}-subgraph"
)


class TemplateHandlersTest(unittest.TestCase):
    def test_manifest_handlers_are_exported(self):
        manifest = (TEMPLATE_DIR / "subgraph.yaml").read_text(encoding="utf-8")
        declared = set(re.findall(r"handler:\s*(\w+)", manifest))

        exported = set()
        for source in (TEMPLATE_DIR / "src").glob("*.ts"):
            exported |= set(
                re.findall(
                    r"export\s+function\s+(\w+)", source.read_text(encoding="utf-8")
                )
            )

        missing = sorted(declared - exported)
        self.assertFalse(
            missing,
            f"Missing handlers in WASM: {', '.join(missing)}",
        )


if __name__ == "__main__":
    unittest.main()
