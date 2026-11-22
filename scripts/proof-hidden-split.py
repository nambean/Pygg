"""Makes hidden-check versions of benchmark cases (ROADMAP gap item 3): the agent and the ladder see half of a case's
one-line tests; the full test file is kept apart for `bench` to run on a passing result (`BENCH_HIDDEN`).

A one-line test is a line that starts with `test(` and ends with `);`. Every second one is removed from the visible
file; multi-line tests stay visible. A case with fewer than six one-line tests is left out.

usage: python scripts/proof-hidden-split.py OUT [case-id-prefix ...]   (default prefix: hard-)
writes OUT/cases/<id>/ (case.json and repo/ with the visible test) and OUT/hidden/<id>/<test path> (the full test)
"""
import json
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ONE_LINE = re.compile(r"^\s*test\(.*\);\s*$")


def main():
    out = Path(sys.argv[1])
    prefixes = sys.argv[2:] or ["hard-"]
    made = []
    for case in sorted((ROOT / "benchmark/cases").iterdir()):
        if not any(case.name.startswith(p) for p in prefixes):
            continue
        spec = json.loads((case / "case.json").read_text(encoding="utf-8"))
        tests = [p for p in spec["protected"] if p.endswith(".js") and "test" in p]
        if not tests:
            continue
        test = tests[0]
        text = (case / "repo" / test).read_text(encoding="utf-8")
        lines = text.split("\n")
        # Balanced on the line: `test('x', () => { const r = f();` opens a block and is not a one-line test.
        singles = [i for i, l in enumerate(lines) if ONE_LINE.match(l) and l.count("{") == l.count("}")
                   and l.count("(") == l.count(")")]
        if len(singles) < 6:
            print(f"{case.name}: {len(singles)} one-line tests, left out")
            continue
        hidden_only = set(singles[1::2])
        visible = "\n".join(l for i, l in enumerate(lines) if i not in hidden_only)
        target = out / "cases" / case.name
        shutil.rmtree(target, ignore_errors=True)
        shutil.copytree(case, target)
        (target / "repo" / test).write_text(visible, encoding="utf-8", newline="")
        hidden = out / "hidden" / case.name / test
        hidden.parent.mkdir(parents=True, exist_ok=True)
        hidden.write_text(text, encoding="utf-8", newline="")
        made.append((case.name, len(singles) - len(hidden_only), len(hidden_only)))
        print(f"{case.name}: {len(singles) - len(hidden_only)} visible one-line tests, {len(hidden_only)} hidden")
    print(f"{len(made)} cases")


if __name__ == "__main__":
    main()
