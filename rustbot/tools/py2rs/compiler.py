"""Emit native Rust control flow and runtime calls, never an AST interpreter."""
from __future__ import annotations

import ast
from dataclasses import dataclass
import types


class UnsupportedSyntax(Exception):
    def __init__(self, filename: str, node: ast.AST, reason: str = ""):
        self.filename = filename
        self.node_type = type(node).__name__
        self.line = getattr(node, "lineno", 1)
        super().__init__(f"{filename}:{getattr(node, 'lineno', 1)}:"
                         f"{getattr(node, 'col_offset', 0) + 1}: unsupported "
                         f"{type(node).__name__}" + (f": {reason}" if reason else ""))


@dataclass(frozen=True)
class Translation:
    rust: str
    metadata: dict


def rust_string(value: str) -> str:
    parts = []
    for char in value:
        if 0xD800 <= ord(char) <= 0xDFFF:
            raise ValueError("Rust strings cannot represent lone Unicode surrogates")
        parts.append({"\\": "\\\\", '"': '\\"', "\n": "\\n", "\r": "\\r", "\t": "\\t"}.get(
            char, f"\\u{{{ord(char):x}}}" if ord(char) < 32 or ord(char) == 127 else char))
    return '"' + "".join(parts) + '"'


def code_spans(code: types.CodeType) -> list[types.CodeType]:
    result = []
    for constant in code.co_consts:
        if isinstance(constant, types.CodeType):
            result.append(constant)
            result.extend(code_spans(constant))
    return result


BINOPS = {ast.Add: "add", ast.Sub: "sub", ast.Mult: "mul", ast.Div: "true_div",
          ast.FloorDiv: "floor_div", ast.Mod: "modulo", ast.Pow: "power"}
BITOPS = {ast.BitAnd: "&", ast.BitOr: "|", ast.BitXor: "^", ast.LShift: "<<", ast.RShift: ">>"}
RUST_KEYWORDS = set("as async await break const continue crate dyn else enum extern false fn for gen if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while abstract become box do final macro override priv typeof unsized virtual yield try".split())


class Compiler:
    def __init__(self, source: str, module: str, filename: str):
        self.source, self.module, self.filename = source, module, filename
        self.tree = ast.parse(source, filename)
        self.code = compile(source, filename, "exec")
        self.codes = code_spans(self.code)
        self.functions: dict[int, dict] = {}
        self.function_nodes: list[ast.FunctionDef | ast.Lambda] = []
        self.classes = {}
        self.comprehensions = {}
        self.counter = 0
        self.in_function = False
        self.frame = "frame"
        self.closure_frame = "frame"
        self.module_scope = True
        self.walrus_frame = None
        self._discover(self.tree, "", False)

    def fail(self, node: ast.AST, reason: str = ""):
        raise UnsupportedSyntax(self.filename, node, reason)

    def temp(self, prefix="v") -> str:
        self.counter += 1
        return f"__{prefix}_{self.counter}"

    def _discover(self, node: ast.AST, parent: str, in_function: bool):
        if isinstance(node, (ast.AsyncFunctionDef, ast.Await, ast.AsyncFor, ast.AsyncWith)):
            self.fail(node, "asynchronous execution is not implemented")
        if isinstance(node, (ast.FunctionDef, ast.Lambda)):
            name = node.name if isinstance(node, ast.FunctionDef) else "<lambda>"
            qualname = (parent + (".<locals>." if in_function is True else ".") if parent else "") + name
            candidates = [c for c in self.codes if c.co_qualname == qualname
                          and c.co_firstlineno in [node.lineno] +
                          [d.lineno for d in getattr(node, "decorator_list", [])]]
            used = {id(info["code"]) for info in self.functions.values()}
            code = next((c for c in candidates if id(c) not in used), None)
            if code is None:
                self.fail(node, "cannot identify CPython code-object span")
            index = len(self.functions)
            rust_name = node.name if isinstance(node, ast.FunctionDef) else "__lambda"
            if rust_name in {"self", "Self", "super", "crate"}: self.fail(node, "function identifier cannot be represented as a Rust identifier")
            if rust_name in RUST_KEYWORDS: rust_name = "r#" + rust_name
            info = {"id": index, "name": name, "rust_name": rust_name,
                    "qualname": qualname, "code": code,
                    "first": code.co_firstlineno,
                    "last": max((line for _, _, line in code.co_lines() if line),
                                default=code.co_firstlineno)}
            self.functions[id(node)] = info
            self.function_nodes.append(node)
            # Defaults/decorators run in the surrounding scope.
            for decorator in getattr(node, "decorator_list", []):
                self._discover(decorator, parent, in_function)
            for default in node.args.defaults + [d for d in node.args.kw_defaults if d is not None]:
                self._discover(default, parent, in_function)
            body = node.body if isinstance(node, ast.FunctionDef) else [node.body]
            for child in body:
                self._discover(child, qualname, True)
            return
        if isinstance(node, ast.ClassDef):
            qualname = (parent + (".<locals>." if in_function is True else ".") if parent else "") + node.name
            self.classes[id(node)] = qualname
            for child in node.body:
                self._discover(child, qualname, False)
            for child in node.bases + [k.value for k in node.keywords] + node.decorator_list:
                self._discover(child, parent, in_function)
            return
        if isinstance(node, ast.GeneratorExp):
            qualname = (parent + (".<locals>." if in_function is True else ".") if parent else "") + "<genexpr>"
            used = {id(info["code"]) for info in self.comprehensions.values()}
            code = next((c for c in self.codes if c.co_qualname == qualname and c.co_firstlineno == node.lineno and id(c) not in used), None)
            if code is None: self.fail(node, "cannot identify generator expression code span")
            self.comprehensions[id(node)] = {"code": code, "qualname": qualname,
                                            "first": code.co_firstlineno,
                                            "last": max((line for _, _, line in code.co_lines() if line), default=code.co_firstlineno)}
            self._discover(node.generators[0].iter, parent, in_function)
            for i, generator in enumerate(node.generators):
                if i: self._discover(generator.iter, qualname, "comprehension")
                for condition in generator.ifs: self._discover(condition, qualname, "comprehension")
            self._discover(node.elt, qualname, "comprehension")
            return
        for child in ast.iter_child_nodes(node):
            self._discover(child, parent, in_function)

    def value(self, value, node: ast.AST) -> str:
        if value is None: return "Value::None"
        if isinstance(value, bool): return f"Value::Bool({str(value).lower()})"
        if isinstance(value, int): return f"Value::Int({rust_string(str(value))}.parse().expect(\"integer literal\"))"
        if isinstance(value, float):
            if value == float("inf"): return "Value::Float(f64::INFINITY)"
            if value == -float("inf"): return "Value::Float(f64::NEG_INFINITY)"
            if value != value: return "Value::Float(f64::NAN)"
            return f"Value::Float({value!r}_f64)"
        if isinstance(value, str):
            try: return f"Value::from({rust_string(value)})"
            except ValueError as error: self.fail(node, str(error))
        if isinstance(value, bytes): return f"pyrt::bytes::BytesObject::value(vec!{list(value)!r})"
        self.fail(node, f"literal type {type(value).__name__}")

    def binary(self, op: ast.operator, a: str, b: str) -> str:
        if type(op) in BINOPS: return f"pyrt::ops::{BINOPS[type(op)]}(&{a}, &{b})?"
        if type(op) in BITOPS: return f"pyrt::ops::bitwise({rust_string(BITOPS[type(op)])}, &{a}, &{b})?"
        self.fail(op)

    def expr(self, node: ast.expr) -> str:
        if isinstance(node, ast.Constant): return self.value(node.value, node)
        if isinstance(node, ast.Name): return f"{self.frame}.load(rt, {rust_string(node.id)})?"
        if isinstance(node, ast.Attribute):
            obj = self.temp()
            return f"{{ let {obj} = {self.expr(node.value)}; rt.getattr(&{obj}, {rust_string(node.attr)})? }}"
        if isinstance(node, ast.BinOp):
            a, b = self.temp(), self.temp()
            return f"{{ let {a} = {self.expr(node.left)}; let {b} = {self.expr(node.right)}; {self.binary(node.op, a, b)} }}"
        if isinstance(node, ast.UnaryOp):
            value = self.temp()
            if isinstance(node.op, ast.Not): operation = f"Value::Bool(!{value}.is_truthy())"
            else:
                name = {ast.UAdd: "pos", ast.USub: "neg", ast.Invert: "invert"}.get(type(node.op))
                if name is None: self.fail(node)
                operation = f"pyrt::ops::{name}(&{value})?"
            return f"{{ let {value} = {self.expr(node.operand)}; {operation} }}"
        if isinstance(node, ast.BoolOp):
            value = self.temp()
            lines = [f"let mut {value} = {self.expr(node.values[0])};"]
            for item in node.values[1:]:
                test = f"{value}.is_truthy()" if isinstance(node.op, ast.And) else f"!{value}.is_truthy()"
                lines.append(f"if {test} {{ {value} = {self.expr(item)}; }}")
            return "{ " + " ".join(lines) + f" {value} }}"
        if isinstance(node, ast.IfExp):
            test = self.temp()
            return f"{{ let {test} = {self.expr(node.test)}; if {test}.is_truthy() {{ {self.expr(node.body)} }} else {{ {self.expr(node.orelse)} }} }}"
        if isinstance(node, ast.Compare):
            left, good = self.temp(), self.temp("matched")
            lines = [f"let mut {left} = {self.expr(node.left)}; let mut {good} = true;"]
            for i, (op, comparator) in enumerate(zip(node.ops, node.comparators)):
                right = self.temp()
                if isinstance(op, ast.Eq): test = f"{left}.equals(&{right})?"
                elif isinstance(op, ast.NotEq): test = f"!{left}.equals(&{right})?"
                elif isinstance(op, ast.Is): test = f"{left}.is_identical(&{right})"
                elif isinstance(op, ast.IsNot): test = f"!{left}.is_identical(&{right})"
                elif isinstance(op, (ast.In, ast.NotIn)):
                    test = f"pyrt::ops::contains(&{right}, &{left})?"
                    if isinstance(op, ast.NotIn): test = "!" + test
                else:
                    relation = {ast.Lt: "o.is_lt()", ast.LtE: "!o.is_gt()", ast.Gt: "o.is_gt()", ast.GtE: "!o.is_lt()"}.get(type(op))
                    if relation is None: self.fail(node)
                    test = f"pyrt::ops::compare(&{left}, &{right})?.is_some_and(|o| {relation})"
                advance = f" {left} = {right};" if i + 1 < len(node.ops) else ""
                lines.append(f"if {good} {{ let {right} = {self.expr(comparator)}; {good} = {test};{advance} }}")
            return "{ " + " ".join(lines) + f" Value::Bool({good}) }}"
        if isinstance(node, ast.Call):
            function, args = self.temp("callable"), self.temp("args")
            lines = [f"let {function} = {self.expr(node.func)}; let mut {args} = CallArgs::default();"]
            for argument in node.args:
                value = self.temp()
                if isinstance(argument, ast.Starred):
                    lines += [f"let {value} = {self.expr(argument.value)};", f"{args}.positional.extend(rt.collect(&{value})?);"]
                else:
                    lines += [f"let {value} = {self.expr(argument)};", f"{args}.positional.push({value});"]
            for keyword in node.keywords:
                value = self.temp()
                lines.append(f"let {value} = {self.expr(keyword.value)};")
                lines.append(f"{args}.extend_keywords(&{value})?;" if keyword.arg is None
                             else f"{args}.keyword({rust_string(keyword.arg)}.into(), {value})?;")
            return "{ " + " ".join(lines) + f" rt.call(&{function}, {args})? }}"
        if isinstance(node, (ast.List, ast.Tuple, ast.Set)):
            result = self.temp("items")
            is_set = isinstance(node, ast.Set)
            lines = [f"let {result} = pyrt::sets::PySet::new();" if is_set else f"let mut {result} = Vec::new();"]
            for item in node.elts:
                value = self.temp()
                if isinstance(item, ast.Starred):
                    lines.append(f"let {value} = {self.expr(item.value)};")
                    if is_set:
                        iterator = self.temp("iter")
                        lines.append(f"let {iterator} = rt.iter(&{value})?; while let Some(v) = rt.next(&{iterator})? {{ {result}.insert(v)?; }}")
                    else: lines.append(f"{result}.extend(rt.collect(&{value})?);")
                else:
                    lines += [f"let {value} = {self.expr(item)};", f"{result}.insert({value})?;" if is_set else f"{result}.push({value});"]
            constructor = "Set" if is_set else "tuple" if isinstance(node, ast.Tuple) else "list"
            return "{ " + " ".join(lines) + f" Value::{constructor}({result}) }}"
        if isinstance(node, ast.Dict):
            result = self.temp("dict")
            lines = [f"let {result} = Value::dict();"]
            for key, item in zip(node.keys, node.values):
                k, value = self.temp(), self.temp()
                if key is None:
                    lines += [f"let {value} = {self.expr(item)};", f"if !matches!({value}, Value::Dict(_)) {{ return Err(PyExc::type_error(\"object is not a mapping\")); }}",
                              f"pyrt::containers::call_method(&{result}, \"update\", &[{value}])?;"]
                else:
                    lines += [f"let {k} = {self.expr(key)}; let {value} = {self.expr(item)}; {result}.dict_set({k}, {value})?;"]
            return "{ " + " ".join(lines) + f" {result} }}"
        if isinstance(node, ast.Subscript):
            obj, index = self.temp(), self.temp()
            if isinstance(node.slice, ast.Slice):
                bounds, setup = self.slice_bounds(node.slice)
                return f"{{ let {obj} = {self.expr(node.value)}; {setup} pyrt::containers::slice(&{obj}, {', '.join(bounds)})? }}"
            return f"{{ let {obj} = {self.expr(node.value)}; let {index} = {self.expr(node.slice)}; pyrt::ops::getitem(&{obj}, &{index})? }}"
        if isinstance(node, ast.NamedExpr):
            value = self.temp()
            if self.walrus_frame is not None:
                assignment = f"{self.walrus_frame}.store({rust_string(node.target.id)}, {value}.clone())?;"
            else: assignment = self.assign(node.target, value + '.clone()')
            return f"{{ let {value} = {self.expr(node.value)}; {assignment} {value} }}"
        if isinstance(node, ast.Lambda): return self.function_value(node)
        if isinstance(node, (ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)):
            return self.comprehension(node)
        if isinstance(node, ast.JoinedStr):
            output = self.temp("text")
            lines = [f"let mut {output} = String::new();"]
            for item in node.values:
                if isinstance(item, ast.Constant): lines.append(f"{output}.push_str({rust_string(item.value)});")
                elif isinstance(item, ast.FormattedValue):
                    value = self.temp()
                    lines.append(f"let {value} = {self.expr(item.value)};")
                    if item.conversion != -1:
                        conv = {ord('s'): "str", ord('r'): "repr", ord('a'): "ascii"}.get(item.conversion)
                        if conv is None: self.fail(item)
                        if conv == "ascii":
                            helper = self.temp()
                            lines += [f"let {helper} = rt.builtin(\"ascii\")?;", f"let {value} = rt.call(&{helper}, CallArgs::new(vec![{value}]))?;"]
                        else: lines.append(f"let {value} = Value::from(pyrt::formatting::{conv}(&{value})?);")
                    spec = self.temp("spec")
                    lines.append(f"let {spec} = {self.expr(item.format_spec) if item.format_spec else 'Value::from(\"\")'};")
                    lines.append(f"{output}.push_str(&pyrt::formatting::format(&{value}, {spec}.as_str()?)?);")
                else: self.fail(item)
            return "{ " + " ".join(lines) + f" Value::from({output}) }}"
        self.fail(node)

    def comprehension(self, node) -> str:
        if any(g.is_async for g in node.generators): self.fail(node, "asynchronous comprehension")
        parent = self.temp("comp_parent")
        previous_walrus = self.walrus_frame
        self.walrus_frame = previous_walrus or parent
        levels = []
        declared = []
        for generator in node.generators:
            declared.extend(n.id for n in ast.walk(generator.target) if isinstance(n, ast.Name))
            iterable = self.expr(generator.iter)
            assignment = self.assign(generator.target, "value")
            filters = [self.expr(condition) for condition in generator.ifs]
            # Each callback owns a clone of the outer walrus destination, rather
            # than borrowing a Rust stack frame that may outlive the generator.
            def callback(body, result, args="rt, frame"):
                capture = f"let {self.walrus_frame} = {self.walrus_frame}.clone();"
                return f"{{ {capture} Rc::new(move |{args}| {{ {body} Ok({result}) }}) }}"
            levels.append("pyrt::comprehension::Level { iterable: " + callback("", iterable)
                          + ", assign: " + callback(assignment, "()", "rt, frame, value")
                          + ", filters: vec![" + ','.join(callback("", f"({condition}).is_truthy()") for condition in filters) + "] }")
        if isinstance(node, ast.DictComp):
            key, value = self.temp(), self.temp()
            emit = f"{{ let {key} = {self.expr(node.key)}; let {value} = {self.expr(node.value)}; Value::tuple(vec![{key},{value}]) }}"
        else: emit = self.expr(node.elt)
        emit_callback = f"{{ let {self.walrus_frame} = {self.walrus_frame}.clone(); Rc::new(move |rt, frame| Ok({emit})) }}"
        self.walrus_frame = previous_walrus
        name = {ast.ListComp: "collect", ast.SetComp: "collect_set", ast.DictComp: "collect_dict", ast.GeneratorExp: "generator_traced"}[type(node)]
        trace = ""
        if isinstance(node, ast.GeneratorExp):
            info = self.comprehensions[id(node)]
            trace = f", Rc::new(|rt| {{ rt.trace_call({rust_string(self.filename)}, {info['first']}, {info['last']}, {rust_string(info['qualname'])}); Ok(()) }})"
        result = f"pyrt::comprehension::{name}(rt, {parent}.clone(), &[{','.join(rust_string(n) for n in sorted(set(declared)))}], vec![{','.join(levels)}], {emit_callback}{trace})?"
        if isinstance(node, ast.ListComp): result = f"Value::list({result})"
        return f"{{ let {parent} = {self.frame}.clone(); {result} }}"

    def slice_bounds(self, node: ast.Slice) -> tuple[list[str], str]:
        bounds, setup = [], []
        for item, default in [(node.lower, "None"), (node.upper, "None"), (node.step, "1_i64")]:
            if item is None:
                bounds.append(default)
            else:
                value, integer = self.temp(), self.temp("index")
                setup += [f"let {value} = {self.expr(item)};", f"let {integer} = if matches!({value}, Value::None) {{ None }} else {{ let n = {value}.as_int()?; Some(n.to_i64().unwrap_or(if n.is_negative() {{ i64::MIN }} else {{ i64::MAX }})) }};"]
                bounds.append(f"{integer}.unwrap_or(1)" if item is node.step else integer)
        return bounds, " ".join(setup)

    def assign(self, target: ast.expr, value: str) -> str:
        if isinstance(target, ast.Name): return f"{self.frame}.store({rust_string(target.id)}, {value})?;"
        if isinstance(target, (ast.Tuple, ast.List)):
            stars = [i for i, item in enumerate(target.elts) if isinstance(item, ast.Starred)]
            before = stars[0] if stars else len(target.elts)
            after = len(target.elts) - before - 1 if stars else 0
            unpacked = self.temp("unpack")
            lines = [f"let {unpacked} = pyrt::containers::unpack(rt, &{value}, {before}, {after}, {str(bool(stars)).lower()})?;"]
            for i, item in enumerate(target.elts):
                lines.append(self.assign(item.value if isinstance(item, ast.Starred) else item, f"{unpacked}[{i}].clone()"))
            return " ".join(lines)
        if isinstance(target, ast.Attribute):
            obj = self.temp()
            return f"let {obj} = {self.expr(target.value)}; rt.setattr(&{obj}, {rust_string(target.attr)}, {value})?;"
        if isinstance(target, ast.Subscript):
            obj, index = self.temp(), self.temp()
            if isinstance(target.slice, ast.Slice):
                bounds, setup = self.slice_bounds(target.slice)
                replacement = self.temp("replacement")
                return f"let {obj} = {self.expr(target.value)}; {setup} let {replacement} = Value::list(rt.collect(&{value})?); pyrt::containers::setslice(&{obj}, {', '.join(bounds)}, &{replacement})?;"
            return f"let {obj} = {self.expr(target.value)}; let {index} = {self.expr(target.slice)}; pyrt::containers::setitem(&{obj}, &{index}, {value})?;"
        self.fail(target)

    def body(self, statements: list[ast.stmt]) -> str:
        return "\n".join(self.statement(statement) for statement in statements)

    def statement(self, node: ast.stmt) -> str:
        if isinstance(node, ast.Pass): return ""
        if isinstance(node, (ast.Global, ast.Nonlocal)): return ""
        if isinstance(node, ast.Expr):
            # CPython elides constant expression statements, including function
            # docstrings. Metadata stores the docstring without per-call copies.
            return "" if isinstance(node.value, ast.Constant) else f"let _ = {self.expr(node.value)};"
        if isinstance(node, ast.Assign):
            value = self.temp()
            return f"let {value} = {self.expr(node.value)};\n" + "\n".join(self.assign(t, value + ".clone()") for t in node.targets)
        if isinstance(node, ast.AugAssign):
            target = node.target
            previous, rhs, result = self.temp("previous"), self.temp("rhs"), self.temp("updated")
            if isinstance(target, ast.Name):
                setup = f"let {previous} = {self.expr(target)};"
                store = self.assign(target, result)
            elif isinstance(target, ast.Attribute):
                obj = self.temp("receiver")
                setup = f"let {obj} = {self.expr(target.value)}; let {previous} = rt.getattr(&{obj}, {rust_string(target.attr)})?;"
                store = f"rt.setattr(&{obj}, {rust_string(target.attr)}, {result})?;"
            elif isinstance(target, ast.Subscript):
                obj, index = self.temp("receiver"), self.temp("index")
                if isinstance(target.slice, ast.Slice): self.fail(node, "augmented slice assignment")
                setup = f"let {obj} = {self.expr(target.value)}; let {index} = {self.expr(target.slice)}; let {previous} = pyrt::ops::getitem(&{obj}, &{index})?;"
                store = f"pyrt::containers::setitem(&{obj}, &{index}, {result})?;"
            else: self.fail(target)
            symbol = {ast.Add: "+", ast.Sub: "-", ast.Mult: "*", ast.Div: "/", ast.FloorDiv: "//", ast.Mod: "%", ast.Pow: "**", **BITOPS}.get(type(node.op))
            if symbol is None: self.fail(node)
            return setup + f" let {rhs} = {self.expr(node.value)}; let {result} = pyrt::ops::inplace(rt, {rust_string(symbol)}, &{previous}, &{rhs})?; " + store
        if isinstance(node, ast.Delete): return '\n'.join(self.delete(t) for t in node.targets)
        if isinstance(node, ast.AnnAssign):
            self.fail(node, "lazy annotation namespaces are not implemented")
        if isinstance(node, ast.Return): return f"return Ok(__Flow::Return({self.expr(node.value) if node.value else 'Value::None'}));"
        if isinstance(node, ast.Break): return "return Ok(__Flow::Break);"
        if isinstance(node, ast.Continue): return "return Ok(__Flow::Continue);"
        if isinstance(node, ast.If):
            test = self.temp()
            return f"let {test} = {self.expr(node.test)}; if {test}.is_truthy() {{\n{self.body(node.body)}\n}} else {{\n{self.body(node.orelse)}\n}}"
        if isinstance(node, (ast.While, ast.For)):
            broke, flow = self.temp("broke"), self.temp("flow")
            prefix = f"let mut {broke} = false;"
            if isinstance(node, ast.For):
                sequence, iterator, item = self.temp(), self.temp("iter"), self.temp()
                prefix += f" let {sequence} = {self.expr(node.iter)}; let {iterator} = rt.iter(&{sequence})?;"
                loop = f"while let Some({item}) = rt.next(&{iterator})?"
                assignment = self.assign(node.target, item)
            else:
                loop = f"while ({self.expr(node.test)}).is_truthy()"
                assignment = ""
            return (prefix + f"\n{loop} {{ {assignment}\nlet {flow} = (|| -> PyResult<__Flow> {{\n{self.body(node.body)}\nOk(__Flow::Next)\n}})()?;\n"
                    + f"match {flow} {{ __Flow::Break => {{ {broke} = true; break; }}, __Flow::Next | __Flow::Continue => {{}}, other => return Ok(other) }}\n}}\n"
                    + f"if !{broke} {{\n{self.body(node.orelse)}\n}}")
        if isinstance(node, ast.FunctionDef): return self.assign(ast.Name(id=node.name), self.function_value(node))
        if isinstance(node, ast.ClassDef): return self.assign(ast.Name(id=node.name), self.class_value(node))
        if isinstance(node, ast.Try): return self.try_statement(node)
        if isinstance(node, ast.Raise):
            if node.cause is not None: self.fail(node, "exception chaining metadata")
            if node.exc is None: return "return Err(rt.active_exception()?);"
            value, exception = self.temp("raised"), self.temp("exception")
            return f"let {value} = {self.expr(node.exc)}; let {exception} = pyrt::exceptions::raised(rt, &{value})?; return Err({exception});"
        if isinstance(node, ast.Assert):
            test, constructor, exception = self.temp(), self.temp("assertion"), self.temp("exception")
            args = f"vec![{self.expr(node.msg)}]" if node.msg else "vec![]"
            return f"let {test} = {self.expr(node.test)}; if !{test}.is_truthy() {{ let {constructor} = rt.builtin(\"AssertionError\")?; let {exception} = rt.call(&{constructor}, CallArgs::new({args}))?; return Err(pyrt::exceptions::raised(rt, &{exception})?); }}"
        if isinstance(node, ast.Import):
            lines = []
            for alias in node.names:
                imported = self.temp("module")
                lines.append(f"let {imported} = rt.import({rust_string(alias.name)})?;")
                if not alias.asname and '.' in alias.name:
                    lines.append(f"let {imported} = rt.import({rust_string(alias.name.split('.')[0])})?;")
                lines.append(f"{self.frame}.store({rust_string(alias.asname or alias.name.split('.')[0])}, {imported})?;")
            return "\n".join(lines)
        if isinstance(node, ast.ImportFrom):
            parts = self.module.split('.') if self.filename.endswith("/__init__.py") else self.module.split('.')[:-1]
            if node.level:
                if node.level > len(parts): self.fail(node, "relative import beyond package")
                module = '.'.join(parts[:len(parts) - node.level + 1] + ([node.module] if node.module else []))
            else: module = node.module or ""
            imported = self.temp("module")
            lines = [f"let {imported} = rt.import({rust_string(module)})?;"]
            for alias in node.names:
                if alias.name == '*': self.fail(node, "star import requires an explicit export table")
                value = self.temp()
                lines += [f"let {value} = rt.import_from({rust_string(module)}, {rust_string(alias.name)})?;", f"{self.frame}.store({rust_string(alias.asname or alias.name)}, {value})?;"]
            return "\n".join(lines)
        self.fail(node)

    def delete(self, target: ast.expr) -> str:
        if isinstance(target, ast.Name): return f"{self.frame}.delete({rust_string(target.id)})?;"
        if isinstance(target, (ast.List, ast.Tuple)): return ' '.join(self.delete(t) for t in target.elts)
        if isinstance(target, ast.Subscript):
            obj, index = self.temp(), self.temp()
            if isinstance(target.slice, ast.Slice):
                bounds, setup = self.slice_bounds(target.slice)
                return f"let {obj} = {self.expr(target.value)}; {setup} pyrt::containers::delslice(&{obj}, {', '.join(bounds)})?;"
            return f"let {obj} = {self.expr(target.value)}; let {index} = {self.expr(target.slice)}; pyrt::containers::delitem(&{obj}, &{index})?;"
        self.fail(target, "attribute deletion is not implemented")

    def block_result(self, body: list[ast.stmt]) -> str:
        return "(|| -> PyResult<__Flow> {\n" + self.body(body) + "\nOk(__Flow::Next)\n})()"

    def try_statement(self, node: ast.Try) -> str:
        outcome, exception = self.temp("outcome"), self.temp("exception")
        result = self.block_result(node.body)
        if node.handlers:
            handlers = []
            for handler in node.handlers:
                condition = f"pyrt::exceptions::matches(&{exception}, &({self.expr(handler.type)}))?" if handler.type else "true"
                body_result = self.block_result(handler.body)
                if handler.name:
                    handled = self.temp("handled")
                    body_result = (f"{{ {self.frame}.store({rust_string(handler.name)}, pyrt::exceptions::value(&{exception}))?; "
                                   f"let {handled} = {body_result}; {self.frame}.store({rust_string(handler.name)}, Value::None)?; "
                                   f"{self.frame}.delete({rust_string(handler.name)})?; {handled} }}")
                handlers.append(f"if {condition} {{ {body_result} }} else ")
            handler_result = ''.join(handlers) + f"{{ Err({exception}.clone()) }}"
            result = (f"match {result} {{\nOk(__Flow::Next) => {self.block_result(node.orelse)},\n"
                      f"Ok(flow) => Ok(flow),\nErr({exception}) => rt.with_exception({exception}.clone(), |rt| {{ {handler_result} }}),\n}}")
        elif node.orelse:
            self.fail(node, "try else requires exception handlers")
        lines = [f"let {outcome}: PyResult<__Flow> = {result};"]
        if node.finalbody:
            final = self.temp("finally")
            final_body = self.block_result(node.finalbody)
            lines.append(f"let {final} = match &{outcome} {{ Err(exc) => rt.with_exception(exc.clone(), |rt| {final_body}), Ok(_) => {final_body} }};")
            lines.append(f"let {outcome} = match {final} {{ Ok(__Flow::Next) => {outcome}, other => other }};")
        flow = self.temp("flow")
        lines.append(f"let {flow} = {outcome}?; if !matches!({flow}, __Flow::Next) {{ return Ok({flow}); }}")
        return '\n'.join(lines)

    def class_value(self, node: ast.ClassDef) -> str:
        if node.keywords or getattr(node, "type_params", []): self.fail(node, "class keywords/metaclasses/generic parameters")
        lines, decorators = [], []
        for decorator in node.decorator_list:
            value = self.temp("decorator")
            lines.append(f"let {value} = {self.expr(decorator)};")
            decorators.append(value)
        bases = []
        for base in node.bases:
            if isinstance(base, ast.Starred): self.fail(base, "starred class base")
            value = self.temp("base")
            lines.append(f"let {value} = {self.expr(base)};")
            bases.append(value)
        namespace, outer = self.temp("namespace"), self.temp("class_outer")
        lines.append(f"let {namespace} = Value::dict();")
        for name, value in [("__module__", self.module), ("__qualname__", self.classes[id(node)]), ("__doc__", ast.get_docstring(node, clean=False))]:
            lines.append(f"{namespace}.dict_set(Value::from({rust_string(name)}), {self.value(value, node)})?;")
        lines.append(f"let {outer} = {self.frame}.clone();")
        previous_closure, previous_scope = self.closure_frame, self.module_scope
        self.closure_frame, self.module_scope = outer, False
        body = self.body(node.body)
        self.closure_frame, self.module_scope = previous_closure, previous_scope
        lines.append(f"{{ let frame = Frame::function({outer}.clone(), &[], &[], &[], {namespace}.clone()); let flow = (|| -> PyResult<__Flow> {{ {body} Ok(__Flow::Next) }})()?; if !matches!(flow, __Flow::Next) {{ return Err(PyExc::runtime_error(\"invalid class control flow\")); }} }}")
        result = self.temp("class")
        lines.append(f"let mut {result} = pyrt::classes::create({rust_string(node.name)}, {rust_string(self.module)}, vec![{','.join(bases)}], {namespace}, rt)?;")
        for decorator in reversed(decorators): lines.append(f"{result} = rt.call(&{decorator}, CallArgs::new(vec![{result}]))?;")
        return "{ " + ' '.join(lines) + f" {result} }}"

    def declarations(self, node: ast.FunctionDef | ast.Lambda) -> tuple[list[str], list[str]]:
        global_names, nonlocal_names = [], []
        def visit(item):
            if isinstance(item, ast.Global): global_names.extend(item.names)
            elif isinstance(item, ast.Nonlocal): nonlocal_names.extend(item.names)
            elif not isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
                for child in ast.iter_child_nodes(item): visit(child)
        for item in node.body if isinstance(node, ast.FunctionDef) else [node.body]: visit(item)
        return sorted(set(global_names)), sorted(set(nonlocal_names))

    def local_bindings(self, node: ast.FunctionDef | ast.Lambda) -> set[str]:
        names = {a.arg for a in node.args.posonlyargs + node.args.args + node.args.kwonlyargs}
        names.update(a.arg for a in [node.args.vararg, node.args.kwarg] if a)
        def visit(item):
            if isinstance(item, ast.Name) and isinstance(item.ctx, (ast.Store, ast.Del)):
                names.add(item.id)
            elif isinstance(item, (ast.FunctionDef, ast.ClassDef)):
                names.add(item.name)
                for child in getattr(item, "decorator_list", []): visit(child)
                if isinstance(item, ast.FunctionDef):
                    for child in item.args.defaults + [d for d in item.args.kw_defaults if d is not None]: visit(child)
                else:
                    for child in item.bases + [k.value for k in item.keywords]: visit(child)
            elif isinstance(item, ast.Lambda):
                for child in item.args.defaults + [d for d in item.args.kw_defaults if d is not None]: visit(child)
            elif isinstance(item, (ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)):
                for generator in item.generators:
                    visit(generator.iter)
                    for condition in generator.ifs: visit(condition)
                for child in ([item.key, item.value] if isinstance(item, ast.DictComp) else [item.elt]): visit(child)
            elif isinstance(item, (ast.Import, ast.ImportFrom)):
                for alias in item.names: names.add(alias.asname or alias.name.split('.')[0])
            else:
                if isinstance(item, ast.ExceptHandler) and item.name: names.add(item.name)
                for child in ast.iter_child_nodes(item): visit(child)
        for statement in node.body if isinstance(node, ast.FunctionDef) else [node.body]: visit(statement)
        return names

    def function_value(self, node: ast.FunctionDef | ast.Lambda) -> str:
        info = self.functions[id(node)]
        if getattr(node, "type_params", []): self.fail(node, "generic type parameters")
        arguments = node.args.posonlyargs + node.args.args + node.args.kwonlyargs
        if getattr(node, "returns", None) or any(a.annotation for a in arguments) or any(a.annotation for a in [node.args.vararg, node.args.kwarg] if a):
            self.fail(node, "lazy function annotations are not implemented")
        capture, signature, result = self.temp("closure"), self.temp("signature"), self.temp("function")
        lines = []
        decorators = []
        for decorator in getattr(node, "decorator_list", []):
            name = self.temp("decorator")
            lines.append(f"let {name} = {self.expr(decorator)};")
            decorators.append(name)
        positional = node.args.posonlyargs + node.args.args
        default_names = []
        for default in node.args.defaults:
            name = self.temp("default")
            lines.append(f"let {name} = {self.expr(default)};")
            default_names.append(name)
        kw_defaults = []
        for arg, default in zip(node.args.kwonlyargs, node.args.kw_defaults):
            if default is None: value = "None"
            else:
                name = self.temp("default")
                lines.append(f"let {name} = {self.expr(default)};")
                value = f"Some({name})"
            kw_defaults.append(f"({rust_string(arg.arg)}.into(), {value})")
        optional_name = lambda arg: f"Some({rust_string(arg.arg)}.into())" if arg else "None"
        lines.append(f"let {signature} = Signature {{ name: {rust_string(info['name'])}.into(), positional: vec![{','.join(rust_string(a.arg)+'.into()' for a in positional)}], positional_only: {len(node.args.posonlyargs)}, defaults: vec![{','.join(default_names)}], keyword_only: vec![{','.join(kw_defaults)}], varargs: {optional_name(node.args.vararg)}, varkw: {optional_name(node.args.kwarg)} }};")
        attributes, kwdict = self.temp("attrs"), self.temp("kwdefaults")
        doc = ast.get_docstring(node, clean=False) if isinstance(node, ast.FunctionDef) else None
        lines.append(f"let {attributes} = Value::dict(); {attributes}.dict_set(Value::from(\"__doc__\"), {self.value(doc, node)})?;")
        defaults = f"Value::tuple({signature}.defaults.clone())" if default_names else "Value::None"
        lines.append(f"{attributes}.dict_set(Value::from(\"__defaults__\"), {defaults})?;")
        lines.append(f"let {kwdict} = Value::dict();")
        # The Signature owns default objects; metadata shares those same values.
        for i, (arg, default) in enumerate(zip(node.args.kwonlyargs, node.args.kw_defaults)):
            if default is not None:
                lines.append(f"{kwdict}.dict_set(Value::from({rust_string(arg.arg)}), {signature}.keyword_only[{i}].1.as_ref().unwrap().clone())?;")
        lines.append(f"{attributes}.dict_set(Value::from(\"__kwdefaults__\"), {kwdict if any(d is not None for d in node.args.kw_defaults) else 'Value::None'})?;")
        captured = f"{self.closure_frame}.clone()" if info['code'].co_freevars else f"Frame::module({self.closure_frame}.globals.clone())"
        lines.append(f"let {capture} = {captured};")
        path = ("super::" if self.in_function else "") + f"__py_{info['id']}::{info['rust_name']}"
        lines.append(f"let mut {result} = NativeFunction::qualified_with_attrs({rust_string(info['name'])}.into(), {rust_string(info['qualname'])}.into(), {rust_string(self.module)}.into(), {attributes}, move |rt, args| {path}(rt, {capture}.clone(), &{signature}, args));")
        for decorator in reversed(decorators):
            lines.append(f"{result} = rt.call(&{decorator}, CallArgs::new(vec![{result}]))?;")
        return "{ " + " ".join(lines) + f" {result} }}"

    def emit_function(self, node: ast.FunctionDef | ast.Lambda) -> str:
        info = self.functions[id(node)]
        code = info["code"]
        globals_, nonlocals = self.declarations(node)
        declared = sorted(self.local_bindings(node) - set(globals_) - set(nonlocals))
        self.in_function, self.module_scope = True, False
        self.frame = self.closure_frame = "frame"
        strings = lambda values: ",".join(rust_string(v) for v in values)
        header = (f"#[allow(clippy::all)]\nmod __py_{info['id']} {{ use super::*;\n"
                f"// py: {self.filename}:{info['first']}-{info['last']} {info['qualname']}\n"
                f"pub(super) fn {info['rust_name']}(rt: &mut Runtime, closure: Rc<Frame>, signature: &Signature, args: CallArgs) -> PyResult<Value> {{\n"
                f"let frame = Frame::function(closure, &[{strings(declared)}], &[{strings(globals_)}], &[{strings(nonlocals)}], signature.bind(args)?);\n")
        if code.co_flags & 0x20:
            from .generators import emit_generator
            return header + emit_generator(self, node, info) + "\n}\n}\n"
        body = self.body(node.body) if isinstance(node, ast.FunctionDef) else f"return Ok(__Flow::Return({self.expr(node.body)}));"
        return (header +
                f"rt.trace_call({rust_string(self.filename)}, {info['first']}, {info['last']}, {rust_string(info['qualname'])});\n"
                f"let flow = (|| -> PyResult<__Flow> {{\n{body}\nOk(__Flow::Next)\n}})()?;\n"
                f"match flow {{ __Flow::Return(value) => Ok(value), __Flow::Next => Ok(Value::None), _ => Err(PyExc::runtime_error(\"invalid loop control outside a loop\")) }}\n"
                "}\n}\n")

    def render(self) -> Translation:
        self.in_function, self.module_scope = False, True
        self.frame = self.closure_frame = "frame"
        module_body = self.body(self.tree.body)
        functions = [self.emit_function(node) for node in self.function_nodes]
        metadata = {"module": self.module, "source": self.filename,
                    "functions": [{k: v for k, v in info.items() if k != "code"}
                                  for info in self.functions.values()],
                    "initializer": "INIT",
                    "imports": sorted({n.name for node in ast.walk(self.tree) if isinstance(node, ast.Import) for n in node.names}),
                    "trace_spans": [{"qualname": c.co_qualname, "first": c.co_firstlineno,
                                     "last": max((line for _, _, line in c.co_lines() if line), default=c.co_firstlineno)} for c in self.codes]}
        spans = ',\n'.join(f"({rust_string(i['qualname'])},{i['first']},{i['last']})" for i in metadata['trace_spans'])
        rust = (f"// @generated by py2rs from {self.filename}\n"
                "// Generated evaluation steps intentionally retain Python sequencing and flow joins.\n"
                "// Hand-written runtime and adapters continue to use strict Clippy checks.\n"
                "#![allow(unused_imports, unused_variables, unused_mut, unused_assignments, unreachable_code, dead_code)]\n"
                "use pyrt::{Value, PyExc, PyResult};\nuse pyrt::runtime::Runtime;\nuse pyrt::frame::Frame;\n"
                "use pyrt::object::{CallArgs, NativeFunction, Signature};\nuse num_traits::{Signed,ToPrimitive};\nuse std::rc::Rc;\n"
                "use pyrt::flow::Flow as __Flow;\n"
                f"pub const PY_MODULE: &str = {rust_string(self.module)};\npub const PY_FUNCTIONS: &[(&str,u32,u32)] = &[{spans}];\n"
                "#[allow(clippy::all)]\npub const INIT: pyrt::runtime::ModuleInit = |rt, module| {\n"
                "let globals = rt.getattr(module, \"__dict__\")?;\nlet frame = Frame::module(globals);\n"
                f"frame.store(\"__doc__\", {self.value(ast.get_docstring(self.tree, clean=False), self.tree)})?;\n"
                f"let flow = (|| -> PyResult<__Flow> {{\n{module_body}\nOk(__Flow::Next)\n}})()?;\n"
                "match flow { __Flow::Next => Ok(()), _ => Err(PyExc::runtime_error(\"invalid module control flow\")) }\n};\n"
                + '\n'.join(functions))
        return Translation(rust, metadata)


def translate(source: str, module: str, filename: str) -> Translation:
    return Compiler(source, module, filename).render()
