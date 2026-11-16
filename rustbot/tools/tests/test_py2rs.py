"""Compile small generated native modules and compare them with CPython."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import warnings

TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parents[1]
sys.path.insert(0, str(TOOLS))
from py2rs import UnsupportedSyntax, translate


SOURCE = '''
counter = []
def record(value):
    counter.append(value)
    return value

def arithmetic(a, b=3):
    chosen = a and b or 7
    return (a + b, a / b, a % b, -a, a < b <= 10, chosen)

def loops(n):
    total = 0
    for i in range(n):
        if i == 2:
            continue
        if i > 5:
            break
        total = total + i
    else:
        total = total + 100
    while n > 0:
        n = n - 1
        if n == 1:
            break
    return total

def make(start, defaults=[]):
    def inner(value=2):
        nonlocal start
        start = start + value
        defaults.append(start)
        return defaults[:]
    return inner

def closures():
    a = make(5)
    first = a()
    second = a(3)
    b = make(1)
    return first, second, b()

def unpack_calls(a, /, *items, flag=True, **kwargs):
    return [a, list(items), flag, kwargs]

def strings(value):
    return f"{value!r:>8} / {3.14159:.2f}"

def ordering():
    return [record(0) and record(1), record(1) or record(2), record(3) < record(2) < record(4), counter]

def comprehensions(n):
    pairs = [(i, j) for i in range(n) if i % 2 for j in range(i) if j < 2]
    mapping = {i: record(i) for i in range(3)}
    values = {i % 3 for i in range(n)}
    stopped = any(record(i) > 1 for i in range(10))
    previous = -1
    assigned = [previous := i for i in range(3)]
    try:
        i
    except NameError:
        scoped = True
    return pairs, mapping, values, stopped, counter, previous, assigned, scoped

def mutations():
    a, *middle, z = range(5)
    values = list(range(7))
    alias = values
    values += [8]
    values[1:5:2] = (i for i in [10, 20])
    del values[::3]
    return a, middle, z, values, alias is values

def decorate(tag):
    record(tag)
    def wrap(function):
        record(tag + 10)
        return function
    return wrap

@decorate(1)
def decorated(value=record(2)):
    """A function docstring."""
    return value

class Counter:
    __slots__ = ("value",)
    def __init__(self, value):
        self.value = value
    def add(self, amount=1):
        self.value += amount
        return self.value

scope_value = 9
class ClassScope:
    scope_value = 50
    def read(self, default=scope_value):
        return scope_value, default

def classes_and_decorators():
    value = Counter(5)
    return value.add(), value.add(3), decorated(), decorated.__doc__, decorated.__defaults__, counter, ClassScope().read()

def reraiser():
    raise

def exceptions_case(mode):
    log = []
    try:
        try:
            if mode == 0:
                return log
            if mode == 1:
                raise ValueError("bad")
            if mode == 2:
                1 / 0
        except (ValueError, TypeError) as error:
            log.append(str(error))
            reraiser()
        else:
            log.append("else")
        finally:
            log.append("finally")
    except Exception as error:
        return log, type(error).__name__
    return log

def final_return():
    try:
        raise ValueError("ignored")
    finally:
        return 7

def deleted_exception_name():
    try:
        raise ValueError("bad")
    except ValueError as error:
        pass
    return error

def keywords():
    return unpack_calls(1, *[2, 3], flag=False, **{"extra": 4})

def generator_values(n):
    record(80)
    for i in range(n):
        if i == 1:
            continue
        if i == 4:
            break
        record(i)
        yield i
    else:
        yield 100
    while n > 0:
        n -= 1
        if n == 2:
            continue
        yield n + 10

def generators(n):
    generator = generator_values(n)
    before = counter[:]
    first = next(generator, None)
    rest = list(generator)
    return before, first, rest, counter

def stops_inside_generator():
    yield 1
    next(iter(()))
    yield 2

def generator_errors():
    values = stops_inside_generator()
    first = next(values)
    try:
        next(values)
    except RuntimeError as error:
        message = str(error)
    return first, message, next(values, "closed")

def traced_generator():
    yield 1

def create_generator():
    traced_generator()
    return "created"

def exhaust_generator():
    return list(traced_generator())

def empty_genexpr():
    return list(x for x in ())
'''

LEAF_SOURCES = {name: (ROOT / f"reference/pybot/bot/understanding/{name}.py").read_text()
                for name in ("car_glass", "cars", "breeds")}


class TranslatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        target = ROOT / "rustbot/target/py2rs-tests"
        target.mkdir(parents=True, exist_ok=True)
        cls.directory = tempfile.TemporaryDirectory(dir=target)
        cls.path = Path(cls.directory.name)
        (cls.path / "src").mkdir()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", SyntaxWarning)
            cls.translation = translate(SOURCE, "fixture", "bot/fixture.py")
            cls.python_code = compile(SOURCE, "fixture", "exec")
        (cls.path / "src/fixture.rs").write_text(cls.translation.rust)
        for name, source in LEAF_SOURCES.items():
            output = translate(source, f"bot.understanding.{name}", f"bot/understanding/{name}.py")
            (cls.path / f"src/{name}.rs").write_text(output.rust)
        (cls.path / "Cargo.toml").write_text('''[package]
name = "py2rs-fixture"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
pyrt = { path = %s }
num-traits = "0.2"
serde_json = "1"
''' % json.dumps(str(ROOT / "rustbot/crates/pyrt")))
        (cls.path / "src/main.rs").write_text('''mod fixture; mod car_glass; mod cars; mod breeds;
use pyrt::{Value,from_canon_json,canon_json,object::CallArgs,runtime::Runtime};
use std::io::{self,BufRead};
fn main(){for line in io::stdin().lock().lines(){let case:serde_json::Value=serde_json::from_str(&line.unwrap()).unwrap();let mut rt=Runtime::default();rt.register_module("fixture",fixture::INIT);rt.register_module("car_glass",car_glass::INIT);rt.register_module("cars",cars::INIT);rt.register_module("breeds",breeds::INIT);let result=(||->pyrt::PyResult<Value>{let module=rt.import(case["module"].as_str().unwrap_or("fixture"))?;let function=rt.getattr(&module,case["name"].as_str().unwrap())?;let positional=from_canon_json(&case["args"])?.sequence()?;if case["trace"].as_bool().unwrap_or(false){rt.begin_trace();}rt.call(&function,CallArgs::new(positional))})();let mut output=match result{Ok(v)=>serde_json::json!({"result":canon_json(&v).unwrap(),"exc":null}),Err(e)=>serde_json::json!({"result":null,"exc":e.kind})};if case["trace"].as_bool().unwrap_or(false){output["calls"]=serde_json::json!(rt.end_trace().iter().map(|s|serde_json::json!([s.qualname,s.first,s.last])).collect::<Vec<_>>());}println!("{}",output);}}
''')
        env = os.environ.copy()
        env["PATH"] = "/home/max/.cargo/bin:" + env.get("PATH", "")
        result = subprocess.run(["cargo", "build", "--manifest-path", str(cls.path / "Cargo.toml"),
                        "--target-dir", str(ROOT / "rustbot/target")],
                       capture_output=True, text=True, timeout=120, env=env)
        if result.returncode:
            raise AssertionError(result.stderr)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def check_cases(self, cases):
        sys.path.insert(0, str(ROOT / "agent/checks"))
        from lib import canon
        payload = [{"name": name, "args": canon.encode(args)} for name, args in cases]
        process = subprocess.run([str(ROOT / "rustbot/target/debug/py2rs-fixture")],
                                 input="".join(json.dumps(case) + "\n" for case in payload),
                                 capture_output=True, text=True, check=True, timeout=15)
        results = [json.loads(line) for line in process.stdout.splitlines()]
        self.assertEqual(len(results), len(cases))
        for (name, args), actual in zip(cases, results):
            namespace = {}
            exec(self.python_code, namespace)
            try:
                expected = {"result": canon.encode(namespace[name](*args)), "exc": None}
            except Exception as error:
                expected = {"result": None, "exc": type(error).__name__}
            self.assertEqual(actual, expected, (name, args))

    def test_native_control_flow_closures_and_values(self):
        self.check_cases([(name, args) for name, args in [
            ("arithmetic", (2,)), ("arithmetic", (0, 0)), ("arithmetic", (-7, 3)),
            ("loops", (4,)), ("loops", (10,)), ("closures", ()),
            ("unpack_calls", (1, 2, 3)), ("strings", ("é",)), ("ordering", ()),
            ("comprehensions", (5,)), ("mutations", ()), ("classes_and_decorators", ()),
            ("exceptions_case", (0,)), ("exceptions_case", (1,)), ("exceptions_case", (2,)), ("exceptions_case", (3,)),
            ("final_return", ()), ("deleted_exception_name", ()), ("keywords", ()),
            ("generators", (0,)), ("generators", (3,)), ("generators", (6,)), ("generator_errors", ())]])

    def test_source_spans_come_from_compiled_code_objects(self):
        functions = self.translation.metadata["functions"]
        self.assertIn("make.<locals>.inner", {f["qualname"] for f in functions})
        for function in functions:
            self.assertIn(f"// py: bot/fixture.py:{function['first']}-{function['last']} {function['qualname']}", self.translation.rust)
        self.assertNotIn("ast::", self.translation.rust)
        self.assertNotIn("eval_ast", self.translation.rust)

    def test_unsupported_syntax_fails_without_stubs(self):
        with self.assertRaisesRegex(UnsupportedSyntax, "AsyncFunctionDef"):
            translate("async def f():\n    return 1\n", "fixture", "bot/fixture.py")
        for source in ["def f():\n    value = yield 1\n", "def f():\n    yield from range(3)\n",
                       "def f():\n    try:\n        yield 1\n    finally:\n        pass\n",
                       "def f():\n    with resource():\n        yield 1\n"]:
            with self.assertRaises(UnsupportedSyntax):
                translate(source, "fixture", "bot/fixture.py")

    def test_generator_traces_resume_only_and_include_empty_final_resume(self):
        cases = [{"name": name, "args": {"$tuple": []}, "trace": True}
                 for name in ("create_generator", "exhaust_generator", "empty_genexpr")]
        process = subprocess.run([str(ROOT / "rustbot/target/debug/py2rs-fixture")],
                                 input="".join(json.dumps(case) + "\n" for case in cases),
                                 text=True, capture_output=True, check=True, timeout=15)
        actual = [json.loads(line) for line in process.stdout.splitlines()]
        names = [[span[0] for span in case["calls"]] for case in actual]
        self.assertEqual(names[0], ["create_generator"])
        self.assertEqual(names[1], ["exhaust_generator", "traced_generator", "traced_generator"])
        self.assertEqual(names[2], ["empty_genexpr", "empty_genexpr.<locals>.<genexpr>"])
        valid_spans = {(span["qualname"], span["first"], span["last"])
                       for span in self.translation.metadata["trace_spans"]}
        self.assertTrue(all(tuple(span) in valid_spans for case in actual for span in case["calls"]))

    def test_cli_strict_failure_is_atomic_and_partial_mode_has_no_stubs(self):
        source = self.path / "cli-source/bot"
        source.mkdir(parents=True)
        (source / "supported.py").write_text("def value():\n    return 42\n")
        (source / "unsupported.py").write_text("async def value():\n    return 42\n")
        output = self.path / "cli-output"
        report = self.path / "cli-report.json"
        command = [sys.executable, "-m", "py2rs", ".", "--source-root", str(source),
                   "--output-root", str(output), "--report", str(report), "--no-rustfmt"]
        env = os.environ.copy(); env["PYTHONPATH"] = str(TOOLS)
        process = subprocess.run(command, env=env, text=True, capture_output=True, timeout=15)
        self.assertEqual(process.returncode, 1)
        self.assertFalse(output.exists())
        coverage = json.loads(report.read_text())
        self.assertEqual((coverage["translated"], coverage["unsupported"]), (1, 1))
        process = subprocess.run(command + ["--keep-going"], env=env, text=True, capture_output=True, timeout=15)
        self.assertEqual(process.returncode, 1)
        self.assertTrue((output / "supported.rs").exists())
        self.assertFalse((output / "unsupported.rs").exists())

    def test_actual_standalone_understanding_leaves(self):
        sys.path.insert(0, str(ROOT / "agent/checks"))
        from lib import canon
        cases = [
            ("car_glass", "_names_one_of", ("car windows mist up", ("mist", "chip"))),
            ("car_glass", "a_window_on_a_car_is_not_a_damage", ({"clean":"why do my car windows mist up"},)),
            ("car_glass", "a_window_on_a_car_is_not_a_damage", ({"clean":"someone smashed my car window"},)),
            ("cars", "_says_a_broken_car", ("my car won't start",)),
            ("cars", "_says_a_broken_car", ("my car is not broken",)),
            ("cars", "a_broken_car_is_the_subject", ("what will it cost", ["my car won't start"])),
            ("cars", "a_broken_car_is_the_subject", ("how much are driving lessons", ["my car won't start"])),
            ("breeds", "the_kind_of", ("greyhounds",)),
            ("breeds", "the_kind_of", ("ragdoll",)),
            ("breeds", "the_animal_a_breed_is", (["my", "greyhound", "is", "not", "eating"],)),
            ("breeds", "the_animal_a_breed_is", (["the", "greyhound", "bus", "is", "late"],)),
            ("breeds", "the_animal_a_breed_is", (["our", "golden", "retriever", "won't", "eat"],)),
        ]
        payload = [{"module": module, "name": name, "args": canon.encode(args)} for module, name, args in cases]
        process = subprocess.run([str(ROOT / "rustbot/target/debug/py2rs-fixture")],
                                 input="".join(json.dumps(c) + "\n" for c in payload),
                                 text=True, capture_output=True, check=True, timeout=30)
        actual = [json.loads(line) for line in process.stdout.splitlines()]
        self.assertEqual(len(actual), len(cases))
        for (module, name, args), result in zip(cases, actual):
            namespace = {"__name__": f"bot.understanding.{module}"}
            exec(LEAF_SOURCES[module], namespace)
            try: expected = {"result": canon.encode(namespace[name](*args)), "exc": None}
            except Exception as error: expected = {"result": None, "exc": type(error).__name__}
            self.assertEqual(result, expected, (module, name, args))


if __name__ == "__main__":
    unittest.main()
