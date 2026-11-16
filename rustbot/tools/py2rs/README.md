# py2rs

`py2rs` compiles Python functions to native Rust functions, operators, loops,
closures and runtime calls. It does not emit an interpreter, embed Python ASTs,
or substitute placeholder functions for unsupported syntax.

Run from the repository root with CPython 3.14 and Rust tools on `PATH`:

```sh
PYTHONPATH=rustbot/tools python3 -m py2rs understanding/car_glass.py \
  --output-root rustbot/target/native-preview \
  --metadata rustbot/target/native-preview/modules.json

PYTHONPATH=rustbot/tools python3 -m py2rs understanding \
  --check --report rustbot/target/understanding-coverage.json

PYTHONPATH=rustbot/tools python3 -m py2rs . \
  --check --report rustbot/target/native-coverage.json
```

The source root defaults to `reference/pybot/bot`. Paths can select individual
files or entire packages. `--source-root` supports isolated source fixtures.
Output paths mirror the source: `__init__.py` maps to `lib.rs` at the root or
`mod.rs` in a package, and other `.py` files map to `.rs`. `bot/__main__.py`
belongs to the native CLI and is omitted.

Every selected file must translate before Rust output is written. Unsupported
syntax produces a source location and a nonzero exit status. `--keep-going`
explicitly permits writing supported **whole files** while retaining a nonzero
status for failures. It never writes shell files for unsupported sources.
Existing files containing `// manual` are protected against replacement.

Each emitted file exposes an `INIT: pyrt::runtime::ModuleInit` function pointer,
`PY_MODULE`, and `PY_FUNCTIONS` source spans. The Python API is
`translate(source, module, filename) -> Translation(rust, metadata)`.
Metadata includes source/module paths, the `INIT` symbol, named functions with
their Rust scope IDs, and trace spans taken from compiled CPython code objects.
Decorators use `co_firstlineno` and the final nonempty `co_lines()` line, matching
the checks' source contract.

Functions retain Python argument binding, definition-time mutable defaults,
closure cells, global/nonlocal writes, short-circuit evaluation and exception
flow. Comprehensions use native callbacks and isolated frames. Generator
expressions and supported generator functions execute lazily; generator bodies
use native Rust resumable control flow. Unsupported suspension constructs,
asynchronous code and lazy annotation namespaces fail explicitly.

Run the small compiled differential fixtures with:

```sh
python3 -m unittest discover -s rustbot/tools/tests -p test_py2rs.py -v
```

Fixtures compile into the repository's `rustbot/target` directory and compare
native outputs with CPython. Inventory generation lives in `../inventory.py`.
