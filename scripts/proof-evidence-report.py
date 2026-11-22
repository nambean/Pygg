"""Tables for the evidence-card proof: caught defects and false alarms per view (A plain, B with the card).

usage: python scripts/proof-evidence-report.py RESULTS.jsonl [...]
"""
import json
import statistics
import sys
from collections import defaultdict

rows = [json.loads(l) for f in sys.argv[1:] for l in open(f, encoding="utf-8") if l.strip()]
by = defaultdict(list)
for r in rows:
    by[(r["reviewer"].split("/")[-1], r["variant"], r["view"])].append(r)
reviewers = sorted({k[0] for k in by})
variants = ["clean", "weakened_test", "out_of_scope", "missing_requirement", "failed_check", "no_check"]
for rev in reviewers:
    print(f"\n{rev}")
    print(f"{'variant':22} {'A caught':>10} {'B caught':>10} {'A secs':>8} {'B secs':>8}")
    tot = {"A": [0, 0], "B": [0, 0]}
    for v in variants:
        cells = []
        for view in ("A", "B"):
            rs = by[(rev, v, view)]
            if v == "clean":
                ok = sum(1 for r in rs if r["verdict"] == "KEEP")
            else:
                ok = sum(1 for r in rs if r["verdict"] == "REJECT" and r["named"])
                tot[view][0] += ok
                tot[view][1] += len(rs)
            cells.append(f"{ok}/{len(rs)}")
        secs = [statistics.median([r["seconds"] for r in by[(rev, v, view)]] or [0]) for view in ("A", "B")]
        label = "clean (kept)" if v == "clean" else v
        print(f"{label:22} {cells[0]:>10} {cells[1]:>10} {secs[0]:>8.1f} {secs[1]:>8.1f}")
    print(f"{'all defects':22} {tot['A'][0]}/{tot['A'][1]:<6} {tot['B'][0]}/{tot['B'][1]:<6}")
