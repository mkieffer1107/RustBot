import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('inventory', Path(__file__).resolve().parents[1] / 'inventory.py')
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class InventoryTests(unittest.TestCase):
    def test_scan_and_trace(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bot = root / 'bot'
            (bot / 'pkg').mkdir(parents=True)
            (bot / '__init__.py').write_text('')
            (bot / 'pkg' / '__init__.py').write_text('')
            (bot / 'pkg' / 'helper.py').write_text('value = 1\n')
            (bot / 'pkg' / 'main.py').write_text(
                'from . import helper as h\nimport re as regex\nfrom math import sqrt as root\n'
                '@h.decorate\ndef respond(x):\n    return regex.search(r"(?=a)", x), root(4), len(x), x.strip()\n')
            trace = root / 'trace'
            trace.mkdir()
            (trace / 'calls.json').write_text(json.dumps({'str.strip': {'count': 2, 'first': 'bot/pkg/main.py:4'}}))
            (trace / 're_patterns.json').write_text(json.dumps([['(?=a)', 0], ['b+', 2]]))
            data, graph = inventory.scan(bot, trace)
            self.assertEqual(data['source_files'], 4)
            self.assertEqual(data['ast_nodes']['Module']['count'], 4)
            self.assertEqual(data['builtins']['len']['count'], 1)
            self.assertEqual(data['stdlib']['math.sqrt']['count'], 1)
            self.assertEqual(data['regex_features']['lookahead']['count'], 1)
            self.assertEqual(data['runtime_trace']['calls']['str.strip'], {'count': 2, 'first': 'bot/pkg/main.py:4'})
            self.assertEqual(data['runtime_trace']['regex_patterns'], [['(?=a)', 0], ['b+', 2]])
            main = graph['bot/pkg/main.py']
            self.assertIn('bot/pkg/helper.py', [x['file'] for x in main['imports']])
            self.assertEqual(main['decorated_responds'][0]['decorators'], ['h.decorate'])
            out = root / 'out'
            inventory.render(data, graph, out)
            self.assertIn('| `str.strip` | 2 | `bot/pkg/main.py:4` |', (out / 'inventory.md').read_text())
            first = (out / 'inventory.json').read_bytes()
            inventory.render(*inventory.scan(bot, trace), out)
            self.assertEqual(first, (out / 'inventory.json').read_bytes())

    def test_missing_trace_is_explicit(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'bot').mkdir()
            (root / 'bot' / '__init__.py').write_text('pass\n')
            data, _ = inventory.scan(root / 'bot', root / 'missing')
            self.assertEqual(data['runtime_trace']['status'], 'missing')
            self.assertIsNone(data['runtime_trace']['calls'])


if __name__ == '__main__':
    unittest.main()
