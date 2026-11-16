#!/usr/bin/env python3
"""Generate a reproducible, conservative static inventory of a Python package."""
import argparse
import ast
import builtins
import json
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
STDLIB = set(sys.stdlib_module_names)
BUILTINS = set(dir(builtins))
REGEX_FEATURES = {
    'lookahead': r'\(\?[=!]', 'lookbehind': r'\(\?<[=!]',
    'named_group': r'\(\?P<', 'named_backreference': r'\(\?P=',
    'numeric_backreference': r'(?<!\\)\\[1-9]', 'conditional': r'\(\?\(',
    'atomic_group': r'\(\?>', 'possessive_quantifier': r'(?:[*+?]|\{\d+(?:,\d*)?\})\+',
    'inline_flags': r'\(\?[aiLmsux-]+[:)]', 'unicode_classes': r'(?<!\\)\\[wWdDsSbB]',
    'end_anchor_Z': r'(?<!\\)\\Z', 'character_class': r'(?<!\\)\[',
}


def dotted(node):
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        base = dotted(node.value)
        return base + '.' + node.attr if base else None
    return None


def module_name(path, package):
    parts = list(path.relative_to(package.parent).with_suffix('').parts)
    if parts[-1] == '__init__':
        parts.pop()
    return '.'.join(parts)


def scan(package, trace_dir):
    package = package.resolve()
    files = sorted(package.rglob('*.py'))
    modules = {module_name(p, package): p for p in files}
    counts = {k: {} for k in ('ast_nodes', 'builtins', 'methods', 'stdlib', 'regex_features')}
    graph = {}

    def add(kind, key, file, line):
        entry = counts[kind].setdefault(key, {'count': 0, 'first': f'{file}:{line}'})
        entry['count'] += 1

    for path in files:
        rel = path.relative_to(package.parent).as_posix()
        name = module_name(path, package)
        tree = ast.parse(path.read_bytes(), filename=rel)
        aliases = {}
        imports = []
        responds = []
        # Import bindings are approximated file-wide: local rebinding and control flow are not resolved.
        for n in ast.walk(tree):
            if isinstance(n, ast.Import):
                for a in n.names:
                    aliases[a.asname or a.name.split('.')[0]] = a.name if a.asname else a.name.split('.')[0]
                    imports.append((a.name, n.lineno))
            elif isinstance(n, ast.ImportFrom):
                pkg = name if path.name == '__init__.py' else name.rpartition('.')[0]
                parts = pkg.split('.') if pkg else []
                if n.level:
                    base = '.'.join(parts[:max(0, len(parts) - n.level + 1)] + ([n.module] if n.module else []))
                else:
                    base = n.module or ''
                imports.append((base, n.lineno))
                for a in n.names:
                    if a.name != '*':
                        target = f'{base}.{a.name}' if base else a.name
                        aliases[a.asname or a.name] = target
                        if target in modules:
                            imports.append((target, n.lineno))
        def resolved(expr):
            text = dotted(expr)
            if not text:
                return None
            head, sep, tail = text.partition('.')
            return aliases.get(head, head) + (sep + tail if sep else '')
        def walk(node, line=1):
            line = getattr(node, 'lineno', line)
            add('ast_nodes', type(node).__name__, rel, line)
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == 'respond' and node.decorator_list:
                responds.append({'line': node.lineno, 'decorators': [ast.unparse(d) for d in node.decorator_list]})
            if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load) and node.id in BUILTINS and node.id not in aliases:
                add('builtins', node.id, rel, line)
            if isinstance(node, ast.Attribute):
                add('methods', node.attr, rel, line)
            if isinstance(node, (ast.Name, ast.Attribute)) and isinstance(node.ctx, ast.Load):
                target = resolved(node)
                if target and target.split('.')[0] in STDLIB and '.' in target:
                    add('stdlib', target, rel, line)
            if isinstance(node, ast.Call):
                target = resolved(node.func)
                if target in {'re.compile', 're.search', 're.match', 're.fullmatch', 're.findall', 're.finditer', 're.sub', 're.subn', 're.split'} and node.args:
                    pattern = node.args[0]
                    if isinstance(pattern, ast.Constant) and isinstance(pattern.value, (str, bytes)):
                        value = pattern.value.decode('latin1') if isinstance(pattern.value, bytes) else pattern.value
                        add('regex_features', 'literal_pattern', rel, line)
                        for feature, detector in REGEX_FEATURES.items():
                            if re.search(detector, value):
                                add('regex_features', feature, rel, line)
                    else:
                        add('regex_features', 'dynamic_pattern', rel, line)
            for child in ast.iter_child_nodes(node):
                walk(child, line)
        walk(tree)
        edges = {}
        for target, line in imports:
            # Importing a package executes its __init__ as well as any explicit submodule.
            bits = target.split('.')
            for i in range(1, len(bits) + 1):
                prefix = '.'.join(bits[:i])
                if prefix in modules:
                    edges.setdefault(modules[prefix].relative_to(package.parent).as_posix(), line)
        graph[rel] = {'imports': [{'file': k, 'line': v} for k, v in sorted(edges.items())],
                      'decorated_responds': sorted(responds, key=lambda x: x['line'])}
    for kind in counts:
        counts[kind] = dict(sorted(counts[kind].items()))
    trace_file = trace_dir / 'calls.json'
    trace = json.loads(trace_file.read_text()) if trace_file.is_file() else None
    patterns_file = trace_dir / 're_patterns.json'
    patterns = json.loads(patterns_file.read_text()) if patterns_file.is_file() else None
    return {'source_files': len(files), **counts,
            'runtime_trace': {'status': 'available' if trace is not None else 'missing', 'calls': trace,
                              'regex_patterns_status': 'available' if patterns is not None else 'missing',
                              'regex_patterns': patterns}}, graph


def render(data, graph, out):
    out.mkdir(parents=True, exist_ok=True)
    (out / 'inventory.json').write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')
    lines = ['# Python source inventory', '', f"Source files: {data['source_files']}. Generated by `python3 rustbot/tools/inventory.py`.", '',
             'Counts are static AST occurrences, with the first source location in sorted file and AST traversal order. Operator and context nodes inherit their containing node’s line.',
             'Builtin and stdlib references are syntactic estimates: imports are resolved file-wide without scope or rebinding analysis. Methods are all attribute names, including module attributes; receiver types are unknown.',
             'Regex features are lexical indicators in literal patterns passed to recognized `re` calls, not a regex parser; dynamic patterns are counted separately. Runtime trace counts are copied as recorded, not static sites. The current golden recorder double-counts inherited counts across seed/clock groups; names and first locations remain useful, but these counts must not be used as profiling weights (see agent/PROGRESS.md, Check disputes).', '',
             f"Runtime call trace: {data['runtime_trace']['status']}; runtime regex patterns: {data['runtime_trace']['regex_patterns_status']}.", '']
    for key in ('ast_nodes', 'builtins', 'methods', 'stdlib', 'regex_features'):
        lines += [f'## {key.replace("_", " ").title()}', '', '| Name | Count | First |', '| --- | ---: | --- |']
        for name, info in data[key].items():
            lines.append(f"| `{name}` | {info['count']} | `{info['first']}` |")
        lines.append('')
    if data['runtime_trace']['calls'] is not None:
        lines += ['## Runtime calls', '', '| Name | Count | First |', '| --- | ---: | --- |']
        for name, info in sorted(data['runtime_trace']['calls'].items()):
            lines.append(f"| `{name}` | {info['count']} | `{info['first']}` |")
        lines.append('')
    (out / 'inventory.md').write_text('\n'.join(lines))
    lines = ['# Import graph', '', 'Direct statically resolved imports within bot, including imported package initializers. Line numbers identify the import statement. Dynamic imports, wildcard exports and rebinding are not resolved. Decorators are shown as source expressions in source order (top to bottom); Python applies them bottom to top.', '']
    for file, info in graph.items():
        lines += [f'## `{file}`', '']
        lines.append('Imports: ' + (', '.join(f"`{x['file']}:{x['line']}`" for x in info['imports']) or 'none'))
        for item in info['decorated_responds']:
            lines.append(f"- `respond` at line {item['line']}: " + ', '.join(f'`{d}`' for d in item['decorators']))
        lines.append('')
    (out / 'import_graph.md').write_text('\n'.join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, default=ROOT / 'reference/pybot/bot')
    parser.add_argument('--trace', type=Path, default=Path(os.environ.get('GOLDENS', ROOT / 'agent/goldens')) / 'trace')
    parser.add_argument('--out', type=Path, default=ROOT / 'rustbot/docs')
    args = parser.parse_args()
    if not args.package.is_dir():
        parser.error(f'package directory missing: {args.package}')
    data, graph = scan(args.package, args.trace)
    render(data, graph, args.out)
    print(f"Scanned {data['source_files']} files; runtime trace {data['runtime_trace']['status']}")


if __name__ == '__main__':
    main()
