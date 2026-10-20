"""Dashed-name alias of `test_rustdoc_guard.py`.

`python3 -m unittest discover -s scripts/quality/tests -p 'test_*.py'` only
imports modules whose file name is a valid Python identifier, so the dashed
file name is skipped by discovery. The canonical implementation therefore
lives in `test_rustdoc_guard.py`; this alias keeps the dashed path usable for
a direct `python3 scripts/quality/tests/test_rustdoc-guard.py` run and runs
exactly the same test cases.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest

_HERE = Path(__file__).resolve().parent
_SPEC = importlib.util.spec_from_file_location(
    "test_rustdoc_guard", _HERE / "test_rustdoc_guard.py"
)
assert _SPEC is not None and _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)

for _name, _value in vars(_MODULE).items():
    if isinstance(_value, type) and issubclass(_value, unittest.TestCase):
        globals()[_name] = _value


if __name__ == "__main__":
    unittest.main()
