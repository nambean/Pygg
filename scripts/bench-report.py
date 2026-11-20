"""Summarizes benchmark result files (JSON lines from `bench`) into Markdown tables with uncertainty.

    python scripts/bench-report.py RESULTS.jsonl [MORE.jsonl ...] [--baseline LABEL-SUBSTRING]

Per arm: runs, passes and the pass rate with a 95% Wilson interval; money (gateway or agent figures, at list
price) in total, per run with a 95% bootstrap interval over cases, and per passed run; median time; for a cascade,
which step solved each passed run. With --baseline, each arm's money per passed run is compared with that arm's,
with a 95% interval from a bootstrap over the cases both arms ran.
"""
import collections, json, math, random, statistics, sys

files, baseline = [], None
args = sys.argv[1:]
while args:
    a = args.pop(0)
    if a == "--baseline":
        baseline = args.pop(0)
    else:
        files.append(a)

records = []
for path in files:
    with open(path, encoding="utf-8") as f:
        records += [json.loads(line) for line in f if line.strip()]

arms = collections.OrderedDict()
for r in records:
    if r.get("stopped") and not r["attempts"]:
        continue  # never started (budget stop); not a run
    arms.setdefault(r["run"], []).append(r)


def money(r):
    vals = [a.get("costMicroUsd") for a in r["attempts"]]
    return sum(v or 0 for v in vals) / 1e6, all(v is not None for v in vals)


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    m = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return ((c - m) / d, (c + m) / d)


def bootstrap_mean_by_case(rs, value, n=2000, seed=7):
    by_case = collections.defaultdict(list)
    for r in rs:
        by_case[r["caseId"]].append(value(r))
    cases = list(by_case)
    rng = random.Random(seed)
    means = []
    for _ in range(n):
        pick = [rng.choice(cases) for _ in cases]
        vals = [v for c in pick for v in by_case[c]]
        means.append(sum(vals) / len(vals))
    means.sort()
    return means[int(0.025 * n)], means[int(0.975 * n)]


def paired_saving(base_rs, arm_rs, n=10000, seed=7):
    """95% bootstrap interval of 1 - (arm's money per passed run / baseline's), resampling the cases both ran,
    so a hard case drawn twice weighs on both arms alike."""
    def by_case(rs):
        d = collections.defaultdict(list)
        for r in rs:
            d[r["caseId"]].append((money(r)[0], r["passed"]))
        return d
    b, a = by_case(base_rs), by_case(arm_rs)
    cases = sorted(set(b) & set(a))
    def pp(d, cs):
        k = sum(ok for c in cs for _, ok in d[c])
        return sum(m for c in cs for m, _ in d[c]) / k if k else float("nan")
    rng = random.Random(seed)
    out = []
    for _ in range(n):
        cs = [rng.choice(cases) for _ in cases]
        base_pp = pp(b, cs)
        if base_pp and not math.isnan(base_pp):
            out.append(1 - pp(a, cs) / base_pp)
    out = sorted(x for x in out if not math.isnan(x))
    return (out[int(0.025 * len(out))], out[int(0.975 * len(out))]) if out else (float("nan"), float("nan"))


rows, per_pass = [], {}
for name, rs in arms.items():
    n = len(rs)
    k = sum(r["passed"] for r in rs)
    lo, hi = wilson(k, n)
    totals = [money(r) for r in rs]
    total = sum(t for t, _ in totals)
    unknown = sum(1 for _, ok in totals if not ok)
    floors = sum(1 for r in rs if any(a.get("costStatus") == "floor" for a in r["attempts"]))
    unknown = f"{unknown} ({floors} floors)" if floors else unknown
    per_run = total / n
    blo, bhi = bootstrap_mean_by_case(rs, lambda r: money(r)[0])
    pp = total / k if k else float("nan")
    per_pass[name] = pp
    med = statistics.median(r["totalMs"] for r in rs) / 1000
    steps = collections.Counter(next((i + 1 for i, a in enumerate(r["attempts"]) if a["passed"]), None) for r in rs if r["passed"])
    rows.append((name, n, k, lo, hi, total, per_run, blo, bhi, pp, med, steps, unknown))

print("| Arm | Runs | Passed | Pass rate (95% CI) | Money | Per run (95% CI) | Per passed run | Median time | Solved at step | Runs with an unreadable cost |")
print("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
for name, n, k, lo, hi, total, per_run, blo, bhi, pp, med, steps, unknown in rows:
    st = ", ".join(f"{s}: {c}" for s, c in sorted(steps.items())) if len(steps) > 1 or name.startswith("engine:cascade") or name.startswith("cascade") else "1"
    print(f"| `{name}` | {n} | {k} | {k / n:.0%} ({lo:.0%} to {hi:.0%}) | ${total:.4f} | ${per_run:.4f} (${blo:.4f} to ${bhi:.4f}) | "
          f"{'$%.4f' % pp if k else '-'} | {med:.0f} s | {st} | {unknown} |")

if baseline:
    base = next((v for kname, v in per_pass.items() if baseline in kname), None)
    if base:
        print()
        print(f"Money per passed run against `{baseline}` (${base:.4f}):")
        print()
        base_name = next(kname for kname in per_pass if baseline in kname)
        for name, pp in per_pass.items():
            if baseline in name or math.isnan(pp):
                continue
            lo, hi = paired_saving(arms[base_name], arms[name])
            quota = sum((a.get("gateway") or {}).get("quota", 0) for r in arms[name] for a in r["attempts"])
            note = f"; it also used {quota} subscription requests, which are quota, not free" if quota else ""
            print(f"- `{name}`: ${pp:.4f}, {1 - pp / base:.1%} less money (95% CI {lo:.1%} to {hi:.1%}, paired by case){note}")

print()
for name, rs in arms.items():
    fails = [r for r in rs if not r["passed"]]
    if fails:
        c = collections.Counter(r["caseId"] for r in fails)
        print(f"**{name}** did not pass: " + ", ".join(f"{k} x{v}" for k, v in sorted(c.items())))
