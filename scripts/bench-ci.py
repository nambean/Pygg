"""Pass rates and cost and tokens per solved task, with 95% confidence intervals, from the bench's raw results.

    python -I scripts/bench-ci.py benchmark/results/2026-11-12-n2/haiku.jsonl [more.jsonl ...] [--json]

Each line of a results file is one run of one case (all its attempts). For each file:
- the pass rate with its Wilson 95% interval;
- cost per solved run = known cost of all runs (failed ones included) / runs passed, with a percentile bootstrap 95%
  interval (10,000 resamples of runs, seeded, so the output is repeatable);
- the same for tokens through the gateway (input including cache reads and writes, and output) per solved run.
A run with an attempt whose cost is unknown is counted, and flagged: its known part only is in the cost.
"""

import json
import math
import random
import sys


def wilson(k, n, z=1.96):
    if n == 0:
        return (None, None)
    p = k / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    r = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return ((c - r) / d, (c + r) / d)


def run_facts(line):
    cost, unknown, tin, tout, cread, cwrite = 0, 0, 0, 0, 0, 0
    for a in line.get("attempts") or []:
        if a.get("costMicroUsd") is not None:
            cost += a["costMicroUsd"]
        if a.get("costStatus") == "unknown":
            unknown += 1
        g = a.get("gateway") or {}
        t = a.get("tokens") or {}
        if g.get("requests"):
            tin += g.get("inputTokens") or 0
            tout += g.get("outputTokens") or 0
            cread += g.get("cacheReadTokens") or 0
            cwrite += g.get("cacheWriteTokens") or 0
        else:
            tin += (t.get("input") or 0) + (t.get("cacheRead") or 0) + (t.get("cacheWrite") or 0)
            tout += t.get("output") or 0
            cread += t.get("cacheRead") or 0
            cwrite += t.get("cacheWrite") or 0
    return {"passed": bool(line.get("passed")), "cost": cost / 1e6, "unknown": unknown, "tokens": tin + tout,
            "input": tin, "output": tout, "cache_read": cread, "cache_write": cwrite}


def per_solved(runs, key):
    solved = sum(r["passed"] for r in runs)
    return sum(r[key] for r in runs) / solved if solved else None


def bootstrap(runs, key, n=10_000, seed=20261115):
    rng = random.Random(seed)
    values = []
    for _ in range(n):
        sample = [runs[rng.randrange(len(runs))] for _ in runs]
        v = per_solved(sample, key)
        if v is not None:
            values.append(v)
    values.sort()
    if not values:
        return (None, None)
    return (values[int(0.025 * len(values))], values[int(0.975 * len(values)) - 1])


def summarise(path):
    runs = [run_facts(json.loads(l)) for l in open(path, encoding="utf-8") if l.strip()]
    k, n = sum(r["passed"] for r in runs), len(runs)
    lo, hi = wilson(k, n)
    total_in = sum(r["input"] for r in runs)
    return {
        "file": path.replace("\\", "/"), "runs": n, "passed": k, "pass_rate": round(k / n, 3) if n else None,
        "pass_rate_ci95": [round(lo, 3), round(hi, 3)] if n else None,
        "cost_usd": round(sum(r["cost"] for r in runs), 4),
        "cost_per_solved_usd": round(per_solved(runs, "cost"), 4) if k else None,
        "cost_per_solved_ci95": [round(x, 4) if x is not None else None for x in bootstrap(runs, "cost")],
        "tokens_per_solved": round(per_solved(runs, "tokens")) if k else None,
        "tokens_per_solved_ci95": [round(x) if x is not None else None for x in bootstrap(runs, "tokens")],
        "share_of_input_from_cache": round(sum(r["cache_read"] for r in runs) / total_in, 3) if total_in else None,
        "runs_with_unknown_cost": sum(1 for r in runs if r["unknown"]),
    }


def main():
    paths = [a for a in sys.argv[1:] if not a.startswith("--")]
    rows = [summarise(p) for p in paths]
    if "--json" in sys.argv:
        for r in rows:
            print(json.dumps(r))
        return
    print("| Results | Passed | 95% CI | $/solved | 95% CI | Tokens/solved | 95% CI | From cache | Unknown cost |")
    print("| --- | --- | --- | ---: | --- | ---: | --- | ---: | ---: |")
    for r in rows:
        ci = r["pass_rate_ci95"]
        c = r["cost_per_solved_ci95"]
        t = r["tokens_per_solved_ci95"]
        money = lambda v: "n/a" if v is None else f"${v:.4f}"
        print(f"| {r['file']} | {r['passed']}/{r['runs']} | {ci[0]:.2f}-{ci[1]:.2f} | {money(r['cost_per_solved_usd'])} | "
              f"{money(c[0])}-{money(c[1])} | {r['tokens_per_solved'] or 'n/a'} | {t[0]}-{t[1]} | "
              f"{'' if r['share_of_input_from_cache'] is None else format(r['share_of_input_from_cache'], '.0%')} | "
              f"{r['runs_with_unknown_cost']} |")


if __name__ == "__main__":
    main()
