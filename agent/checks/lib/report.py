"""Divergence reporting and the final `CHECK <name> PASS|FAIL|MISSING ...` line."""
import sys

EXIT = {"PASS": 0, "FAIL": 1, "MISSING": 2}


class Report:
    def __init__(self, name, max_shown=10):
        self.name, self.max_shown = name, max_shown
        self.total = 0
        self.failed = 0
        self.shown = 0
        self.extra = {}

    def ok(self, n=1):
        self.total += n

    def fail(self, where, detail):
        self.total += 1
        self.failed += 1
        if self.shown < self.max_shown:
            self.shown += 1
            print(f"  DIVERGENCE {where}\n    {detail}", flush=True)

    def finish(self, unit="cases", **numbers):
        """Print the last line and return the exit code."""
        nums = " ".join(f"{k}={v}" for k, v in numbers.items())
        if self.failed:
            status = "FAIL"
            if self.failed > self.shown:
                print(f"  ... {self.failed - self.shown} more divergences not shown", flush=True)
        else:
            status = "PASS"
        pct = 100.0 * (self.total - self.failed) / self.total if self.total else 100.0
        print(f"CHECK {self.name} {status} {unit}={self.total} failed={self.failed} match={pct:.2f}% {nums}".rstrip(),
              flush=True)
        return EXIT[status]


def missing(name, why):
    print(f"  {why}", file=sys.stderr, flush=True)
    print(f"CHECK {name} MISSING {why}", flush=True)
    return EXIT["MISSING"]


def missing_exit(why, name=None):
    """Used by helpers that do not know the check name: the caller's name is taken from CHECK_NAME."""
    import os
    return missing(name or os.environ.get('CHECK_NAME', 'check'), why)


def fail_line(name, why):
    print(f"CHECK {name} FAIL {why}", flush=True)
    return EXIT["FAIL"]
