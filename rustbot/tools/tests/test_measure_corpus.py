"""Lightweight isolation/IPC regression checks; these never load PyBot."""
import importlib.util
import json
from pathlib import Path
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("measure_corpus", ROOT / "rustbot/tools/measure_corpus.py")
TOOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOL)


class MeasurementTests(unittest.TestCase):
    def test_pipe_is_drained_before_waiting_for_large_result(self):
        payload = b"x" * (4 * 1024 * 1024)
        self.assertEqual(TOOL.fork_result(lambda _: payload, None, 10), payload)

    def test_child_exception_is_reported_instead_of_silently_omitted(self):
        def fail(_):
            raise ValueError("failure marker")
        with self.assertRaisesRegex(RuntimeError, "failure marker"):
            TOOL.fork_result(fail, None, 10)

    def test_child_deadline_kills_and_reaps_the_child(self):
        started = time.monotonic()
        with self.assertRaises(TimeoutError):
            TOOL.fork_result(lambda _: time.sleep(10), None, 0.05)
        self.assertLess(time.monotonic() - started, 2)

    def test_corpus_identity_validation_and_atomic_json(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "rustbot/target") as scratch:
            scratch = Path(scratch)
            corpus = scratch / "input.jsonl"
            row = {"id": "one", "seed": 0, "clock": "2026-06-15T12:00:00", "turns": ["hello"]}
            corpus.write_text(json.dumps(row) + "\n")
            rows, fingerprint = TOOL.load_corpus(corpus)
            self.assertEqual(rows, [row])
            self.assertEqual(len(fingerprint), 64)
            corpus.write_text((json.dumps(row) + "\n") * 2)
            with self.assertRaisesRegex(ValueError, "duplicate conversation"):
                TOOL.load_corpus(corpus)
            output = scratch / "out.json"
            TOOL.atomic_json(output, {"complete": True})
            self.assertEqual(json.loads(output.read_text()), {"complete": True})
            self.assertEqual(list(scratch.glob(".coverage-isolated-*")), [])


if __name__ == "__main__":
    unittest.main()
