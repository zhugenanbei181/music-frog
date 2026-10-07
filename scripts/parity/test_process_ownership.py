"""Private launch receipts cannot bind to an inherited process group."""
import subprocess
import unittest
from pathlib import Path

HELPER = Path(__file__).with_name('process_ownership.sh')


class ProcessOwnershipTests(unittest.TestCase):
    def receipt(self, child):
        return subprocess.check_output(
            ['bash', '-c', 'source "$1"; process_group "$2"', 'ownership-test', str(HELPER), str(child.pid)],
            text=True,
        )

    def stop(self, child):
        if child.poll() is None:
            child.terminate()
        child.wait(timeout=2)

    def test_fresh_session_is_bound_to_the_exact_spawned_pid(self):
        for _ in range(5):
            child = subprocess.Popen(['setsid', 'sleep', '2'])
            try:
                self.assertEqual(self.receipt(child), str(child.pid))
            finally:
                self.stop(child)

    def test_an_inherited_group_never_becomes_an_owned_receipt(self):
        child = subprocess.Popen(['sleep', '0.15'])
        try:
            self.assertEqual(self.receipt(child), '')
        finally:
            self.stop(child)
