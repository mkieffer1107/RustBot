"""Lower synchronous Python generators to resumable native Rust states.

This is code generation: every state contains native Rust emitted by Compiler,
not Python AST data or instructions interpreted at run time.
"""
from __future__ import annotations

import ast

from .compiler import rust_string


def _contains_yield(node: ast.AST) -> bool:
    """Find yields belonging to this scope, excluding nested lexical scopes."""
    class Visitor(ast.NodeVisitor):
        found = False

        def visit_Yield(self, node):
            self.found = True

        def visit_YieldFrom(self, node):
            self.found = True

        def visit_FunctionDef(self, node):
            pass

        visit_AsyncFunctionDef = visit_FunctionDef
        visit_Lambda = visit_FunctionDef
        visit_ClassDef = visit_FunctionDef
        visit_GeneratorExp = visit_FunctionDef

    visitor = Visitor()
    visitor.visit(node)
    return visitor.found


def _validate_returns(compiler, body: list[ast.stmt]) -> None:
    """Reject return values until the iterator protocol preserves StopIteration.value."""
    class Visitor(ast.NodeVisitor):
        def visit_Return(self, node):
            if node.value is not None and not (isinstance(node.value, ast.Constant) and node.value.value is None):
                compiler.fail(node, "generator return values are not implemented")
            self.generic_visit(node)

        def visit_FunctionDef(self, node):
            pass

        visit_AsyncFunctionDef = visit_FunctionDef
        visit_Lambda = visit_FunctionDef
        visit_ClassDef = visit_FunctionDef
        visit_GeneratorExp = visit_FunctionDef

    visitor = Visitor()
    for stmt in body:
        visitor.visit(stmt)


class _Generator:
    def __init__(self, compiler):
        self.c = compiler
        self.states: list[str | None] = []
        self.iterators: list[str] = []
        self.done = self.reserve()
        self.set(self.done, "return Ok(None);")

    def reserve(self) -> int:
        result = len(self.states)
        self.states.append(None)
        return result

    def set(self, state: int, text: str) -> None:
        assert self.states[state] is None
        self.states[state] = text

    def state(self, text: str) -> int:
        state = self.reserve()
        self.set(state, text)
        return state

    @staticmethod
    def go(state: int) -> str:
        return f"__gen_pc = {state}; continue;"

    def block(self, body: list[ast.stmt], next_state: int,
              break_state: int | None = None, continue_state: int | None = None) -> int:
        for node in reversed(body):
            next_state = self.statement(node, next_state, break_state, continue_state)
        return next_state

    def statement(self, node: ast.stmt, next_state: int,
                  break_state: int | None, continue_state: int | None) -> int:
        c = self.c
        if isinstance(node, ast.Expr) and isinstance(node.value, ast.Yield):
            value = "Value::None" if node.value.value is None else c.expr(node.value.value)
            return self.state(f"let __yielded = {value}; __gen_pc = {next_state}; return Ok(Some(__yielded));")
        if isinstance(node, ast.Expr) and isinstance(node.value, ast.YieldFrom):
            c.fail(node, "yield from is not implemented")
        if isinstance(node, ast.If):
            if _contains_yield(node.test):
                c.fail(node.test, "yield inside an expression is not implemented")
            yes = self.block(node.body, next_state, break_state, continue_state)
            no = self.block(node.orelse, next_state, break_state, continue_state)
            return self.state(f"if ({c.expr(node.test)}).is_truthy() {{ {self.go(yes)} }} else {{ {self.go(no)} }}")
        if isinstance(node, ast.While):
            if _contains_yield(node.test):
                c.fail(node.test, "yield inside an expression is not implemented")
            check = self.reserve()
            otherwise = self.block(node.orelse, next_state, break_state, continue_state)
            body = self.block(node.body, check, next_state, check)
            self.set(check, f"if ({c.expr(node.test)}).is_truthy() {{ {self.go(body)} }} else {{ {self.go(otherwise)} }}")
            return check
        if isinstance(node, ast.For):
            if _contains_yield(node.iter) or _contains_yield(node.target):
                c.fail(node, "yield inside a for iterable or target is not implemented")
            iterator = c.temp("gen_iter")
            self.iterators.append(iterator)
            check = self.reserve()
            break_cleanup = self.state(f"{iterator} = None; {self.go(next_state)}")
            otherwise = self.block(node.orelse, next_state, break_state, continue_state)
            body = self.block(node.body, check, break_cleanup, check)
            item = c.temp("gen_item")
            assignment = c.assign(node.target, item)
            self.set(check, f"let {item} = rt.next({iterator}.as_ref().ok_or_else(|| PyExc::runtime_error(\"generator iterator missing\"))?)?; "
                     f"match {item} {{ Some({item}) => {{ {assignment} {self.go(body)} }}, "
                     f"None => {{ {iterator} = None; {self.go(otherwise)} }} }}")
            iterable = c.expr(node.iter)
            return self.state(f"let __iterable = {iterable}; {iterator} = Some(rt.iter(&__iterable)?); {self.go(check)}")
        if isinstance(node, ast.Break):
            if break_state is None:
                c.fail(node, "break outside a loop")
            return self.state(self.go(break_state))
        if isinstance(node, ast.Continue):
            if continue_state is None:
                c.fail(node, "continue outside a loop")
            return self.state(self.go(continue_state))
        if isinstance(node, ast.Return):
            if node.value is not None and _contains_yield(node.value):
                c.fail(node, "yield inside a return value is not implemented")
            value = "Value::None" if node.value is None else c.expr(node.value)
            return self.state(f"let _ = {value}; __gen_pc = {self.done}; return Ok(None);")
        if _contains_yield(node):
            if isinstance(node, (ast.Try, ast.TryStar, ast.With, ast.AsyncWith)):
                c.fail(node, "yield inside try/with is not implemented")
            c.fail(node, "yield inside this expression or statement is not implemented")
        # Regular statements still run in a flow-returning closure, so e.g.
        # break/continue/return inside a yield-free try block cross correctly.
        flow = c.temp("gen_flow")
        break_action = self.go(break_state) if break_state is not None else 'return Err(PyExc::runtime_error("break outside a loop"));'
        continue_action = self.go(continue_state) if continue_state is not None else 'return Err(PyExc::runtime_error("continue outside a loop"));'
        return self.state(
            f"let {flow} = {c.block_result([node])}?; match {flow} {{ "
            f"__Flow::Next => {{ {self.go(next_state)} }}, "
            f"__Flow::Return(_) => {{ __gen_pc = {self.done}; return Ok(None); }}, "
            f"__Flow::Break => {{ {break_action} }}, __Flow::Continue => {{ {continue_action} }} }}")

    def render(self, node: ast.FunctionDef | ast.Lambda, info: dict) -> str:
        c = self.c
        if isinstance(node, ast.Lambda):
            if not isinstance(node.body, ast.Yield):
                c.fail(node, "generator lambda with a nested yield expression is not implemented")
            body = [ast.copy_location(ast.Expr(value=node.body), node.body)]
        else:
            body = node.body
            _validate_returns(c, body)
        entry = self.block(body, self.done)
        declarations = "\n".join(f"let mut {iterator}: Option<Value> = None;" for iterator in self.iterators)
        cleanup = " ".join(f"{iterator} = None;" for iterator in self.iterators)
        arms = "\n".join(f"{i} => {{ {code} }}" for i, code in enumerate(self.states))
        assert all(code is not None for code in self.states)
        trace = (f"rt.trace_call_code({rust_string(c.filename)}, {info['first']}, {info['last']}, "
                 f"{rust_string(info['qualname'])}, {info['code_id']});")
        return (f"let mut __gen_pc: usize = {entry};\n{declarations}\n"
                "let mut __gen_frame = Some(frame);\n"
                "Ok(pyrt::object::IteratorObject::value(move |rt| {\n"
                f"if __gen_pc == usize::MAX {{ return Ok(None); }}\n{trace}\n"
                "let frame = __gen_frame.as_ref().expect(\"active generator frame\").clone();\n"
                "let __gen_result: PyResult<Option<Value>> = (|| { loop { match __gen_pc {\n"
                f"{arms}\n_ => return Err(PyExc::runtime_error(\"invalid generator state\")),\n"
                "} } })();\n"
                "match __gen_result {\n"
                f"Err(e) => {{ __gen_pc = usize::MAX; __gen_frame = None; {cleanup} "
                "if e.kind == \"StopIteration\" { Err(PyExc::runtime_error(\"generator raised StopIteration\")) } else { Err(e) } },\n"
                f"Ok(None) => {{ __gen_pc = usize::MAX; __gen_frame = None; {cleanup} Ok(None) }},\n"
                "other => other,\n} }))")


def emit_generator(compiler, node: ast.FunctionDef | ast.Lambda, info: dict) -> str:
    """Return a function body after its argument-bound `frame` initialization."""
    return _Generator(compiler).render(node, info)
