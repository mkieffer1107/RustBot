"""The regex recorder must preserve Python pattern/match identities.

The fixture uses a virtual bot filename to exercise the recorder's caller
filter. It never writes a file into the official reference checkout.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
PROGRAM = r'''
import copy
import re
pattern = re.compile(r"(?P<amount>\d+) pounds")
text = "25 pounds"
matched = pattern.search(text)
observed = []
def replacement(match):
    observed.append(match.re is pattern)
    observed.append(match.re is re.compile(pattern.pattern))
    observed.append(isinstance(match, re.Match))
    return "<" + match["amount"] + ">"
result = {
    "owner": matched.re is pattern,
    "match_type": isinstance(matched, re.Match),
    "group": matched["amount"],
    "copy": copy.copy(matched) is matched,
    "deepcopy": copy.deepcopy(matched) is matched,
    "module_owner": re.search(pattern.pattern, text).re is pattern,
    "iter_owners": [m.re is pattern for m in pattern.finditer(text)],
    "module_iter_owners": [m.re is pattern for m in re.finditer(pattern.pattern, text)],
    "sub": pattern.sub(replacement, text),
    "keyword_sub": pattern.sub(repl=replacement, string=text),
    "module_sub": re.sub(pattern.pattern, replacement, text),
    "subn": pattern.subn(replacement, text),
    "observed": observed,
}
'''

DRIVER = r'''
import json
from pathlib import Path
import sys
sys.path.insert(0, sys.argv[1])
from lib import probes
if sys.argv[2] == "recorded":
    probes.install_re()
rows = []
for enabled in (False, True):
    probes.ReLog.enabled = enabled
    namespace = {}
    code = compile(sys.argv[3], str(Path(probes.BOT_DIR) / "recorder_identity_fixture.py"), "exec")
    exec(code, namespace)
    rows.append(namespace["result"])
callables = sum(any(isinstance(a, dict) and "$callable" in a for a in c["args"])
                for c in probes.ReLog.cases)
print(json.dumps({"rows": rows, "recorded": len(probes.ReLog.cases), "callables": callables}))
'''


class RecorderIdentityTest(unittest.TestCase):
    def run_fixture(self, mode):
        process = subprocess.run(
            [sys.executable, "-s", "-c", DRIVER, str(ROOT / "agent/checks"), mode, PROGRAM],
            env={**os.environ, "PYTHONHASHSEED": "0", "PYTHONDONTWRITEBYTECODE": "1"},
            capture_output=True, text=True, timeout=30,
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        return json.loads(process.stdout)

    def test_recording_preserves_identity_with_logging_on_and_off(self):
        raw = self.run_fixture("raw")
        recorded = self.run_fixture("recorded")
        self.assertEqual(recorded["rows"], raw["rows"])
        for row in recorded["rows"]:
            self.assertTrue(row["owner"] and row["module_owner"] and row["match_type"])
            self.assertTrue(all(row["observed"]))
        self.assertGreater(recorded["recorded"], 0)
        self.assertGreater(recorded["callables"], 0)


if __name__ == "__main__":
    unittest.main()
