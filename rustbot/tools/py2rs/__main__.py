"""Translate only requested source files, failing atomically on unsupported syntax."""
import argparse
from collections import Counter
import json
from pathlib import Path
import shutil
import subprocess
import sys

from .compiler import UnsupportedSyntax, translate


def mirror(relative: Path) -> Path:
    if relative.name == "__main__.py":
        raise ValueError("bot/__main__.py is owned by the native CLI")
    if relative.name == "__init__.py":
        return relative.parent / ("lib.rs" if relative.parent == Path(".") else "mod.rs")
    return relative.with_suffix(".rs")


def main(argv=None) -> int:
    root = Path(__file__).resolve().parents[3]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", help="Python files or package directories below the source root")
    parser.add_argument("--source-root", type=Path, default=root / "reference/pybot/bot")
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--check", action="store_true", help="translate and report without writing Rust files")
    parser.add_argument("--report", type=Path, help="write strict coverage and unsupported syntax details as JSON")
    parser.add_argument("--keep-going", action="store_true", help="write supported whole files even if other selected files fail; exit status remains nonzero")
    parser.add_argument("--no-rustfmt", action="store_true")
    args = parser.parse_args(argv)
    if not args.check and args.output_root is None: parser.error("--output-root is required unless --check is used")
    source_root = args.source_root.resolve()
    inputs = set()
    for requested in args.paths:
        path = Path(requested)
        if not path.is_absolute(): path = source_root / path
        path = path.resolve()
        if not path.is_relative_to(source_root): parser.error(f"source outside source root: {path}")
        if path.is_dir(): inputs.update(path.rglob("*.py"))
        elif path.is_file() and path.suffix == ".py": inputs.add(path)
        else: parser.error(f"not a Python source file or package: {path}")
    translations, failures = [], []
    for source in sorted(inputs):
        try:
            relative = source.relative_to(source_root)
            if relative == Path("__main__.py"): continue
            module_parts = relative.with_suffix("").parts
            if module_parts[-1] == "__init__": module_parts = module_parts[:-1]
            module = '.'.join([source_root.name] + list(module_parts))
            result = translate(source.read_text(encoding="utf-8"), module, f"{source_root.name}/{relative.as_posix()}")
            translations.append((mirror(relative), result))
        except (UnsupportedSyntax, SyntaxError, ValueError) as error:
            failures.append({"source": str(source.relative_to(source_root)), "node": getattr(error, "node_type", type(error).__name__),
                             "line": getattr(error, "line", getattr(error, "lineno", 1)), "error": str(error)})
            print(error, file=sys.stderr)
    metadata = [{"rust_path": str(path), **result.metadata} for path, result in translations]
    if args.metadata:
        args.metadata.parent.mkdir(parents=True, exist_ok=True)
        args.metadata.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps({"selected": len(translations) + len(failures), "skipped_cli": len(inputs) - len(translations) - len(failures), "translated": len(translations),
            "unsupported": len(failures), "unsupported_nodes": dict(Counter(f["node"] for f in failures)),
            "files": [{"source":m["source"],"module":m["module"],"rust_path":m["rust_path"],"functions":len(m["functions"])} for m in metadata],
            "failures": failures}, indent=2) + "\n", encoding="utf-8")
    if args.check:
        print(f"Strict coverage: translated={len(translations)} unsupported={len(failures)} selected={len(translations) + len(failures)}")
        return bool(failures)
    if failures and not args.keep_going:
        print("No Rust files written: one or more selected sources are unsupported", file=sys.stderr)
        return 1
    # No output is written until every selected source has translated cleanly.
    for relative, result in translations:
        output = args.output_root / relative
        if output.exists() and "// manual" in output.read_text(encoding="utf-8"):
            raise SystemExit(f"refusing to overwrite manual markers in {output}")
    formatter = shutil.which("rustfmt")
    if not args.no_rustfmt and formatter is None:
        raise SystemExit("rustfmt is required (or explicitly pass --no-rustfmt)")
    for relative, result in translations:
        output = args.output_root / relative
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(result.rust, encoding="utf-8")
        if not args.no_rustfmt:
            subprocess.run([formatter, "--edition", "2024", str(output)], check=True)
    print(f"Translated {len(translations)} source files to native Rust; unsupported={len(failures)}")
    return bool(failures)


if __name__ == "__main__":
    sys.exit(main())
