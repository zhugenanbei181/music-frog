"""Discovery shim for `test_parity-coverage-guard.py`.

`unittest` discovery only imports files whose name is a valid Python
identifier, so the hyphenated test module is loaded here and its test cases
are re-exported. The cases themselves live in the hyphenated module.
"""

import importlib.util
from pathlib import Path
import sys
import unittest

_SOURCE = Path(__file__).with_name("test_parity-coverage-guard.py")
_SPEC = importlib.util.spec_from_file_location("test_parity_coverage_guard_cases", _SOURCE)
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)

globals().update({
    name: value
    for name, value in vars(_MODULE).items()
    if isinstance(value, type)
    and issubclass(value, unittest.TestCase)
    and value is not unittest.TestCase
})

if __name__ == "__main__":
    unittest.main()
