"""Table for the cheap-first ladder proof: per arm, visible pass rate, hidden pass rate, climbs, tokens and time.

usage: python scripts/proof-ladder-hidden.py RESULTS.jsonl [...]
"""
import json
import statistics
import sys
from collections import defaultdict

by = defaultdict(list)
for f in sys.argv[1:]:
    for line in open(f, encoding="utf-8"):
        if line.strip():
            r = json.loads(line)
            by[r["run"]].append(r)
print(f"{'arm':70} {'visible':>8} {'hidden':>8} {'both':>8} {'climbs':>6} {'tokens/run':>11} {'min/run':>8}")
for run, rs in sorted(by.items()):
    vis = sum(r["passed"] for r in rs)
    hid = sum(1 for r in rs if r.get("hidden_passed") or r.get("hiddenPassed"))
    climbs = sum(1 for r in rs if len(r["attempts"]) > 1)
    tokens = [sum((a.get("gateway") or {}).get("inputTokens", 0) + (a.get("gateway") or {}).get("outputTokens", 0)
                  for a in r["attempts"]) for r in rs]
    mins = statistics.median(r["totalMs"] for r in rs) / 60000
    print(f"{run[:70]:70} {vis:>3}/{len(rs):<4} {hid:>3}/{len(rs):<4} {hid:>3}/{vis:<4} {climbs:>6} "
          f"{statistics.median(tokens):>11,.0f} {mins:>8.1f}")
    for r in rs:
        if r["passed"] and not (r.get("hidden_passed") or r.get("hiddenPassed")):
            print(f"    passed visible, failed hidden: {r['caseId']}")
