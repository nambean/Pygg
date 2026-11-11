"""Paired comparison of task-engine handoff arms (ADR-030, H1) from `bench --engine` result files.

    python scripts/handoff-ab.py ARM=RESULTS.jsonl ARM=RESULTS.jsonl [...]

Each arm must have run the same cases the same number of times; runs are paired by case and round. Per arm it
prints runs, passes, how the first step ended, and step-2 means. For every pair of arms it prints the mean
per-run difference (first minus second) in step-2 time, step-2 gateway input and output tokens and money, with
95% and 99.6% (12 comparisons, Bonferroni) intervals from a paired bootstrap over runs (20,000 resamples, seed 7).
"""
import collections, itertools, json, random, sys

arms = collections.OrderedDict()
for arg in sys.argv[1:]:
    name, path = arg.split("=", 1)
    with open(path, encoding="utf-8") as f:
        arms[name] = [json.loads(line) for line in f if line.strip()]


def later(r, f):
    return sum(f(a) for a in r["attempts"][1:])


METRICS = [
    ("step-2 time (ms)", lambda a: a["agentMs"]),
    ("step-2 input tokens", lambda a: (a.get("gateway") or {}).get("inputTokens", 0)),
    ("step-2 output tokens", lambda a: (a.get("gateway") or {}).get("outputTokens", 0)),
    ("money (micro-USD)", lambda a: a.get("costMicroUsd") or 0),
]

for name, recs in arms.items():
    first = collections.Counter(r["attempts"][0]["agentExit"] for r in recs)
    checked = sum(1 for r in recs if r["attempts"][0]["checks"])
    print(f"{name}: {sum(r['passed'] for r in recs)}/{len(recs)} passed; step 1 ended {dict(first)}; "
          f"step 1 reached the checks in {checked}")
    for label, f in METRICS:
        print(f"  mean {label}: {sum(later(r, f) for r in recs) / len(recs):.0f}")


def paired(recs):
    seen, out = collections.Counter(), {}
    for r in recs:
        seen[r["caseId"]] += 1
        out[(r["caseId"], seen[r["caseId"]])] = r
    return out


keyed = {n: paired(r) for n, r in arms.items()}
common = sorted(set.intersection(*(set(k) for k in keyed.values())))
print(f"\npaired runs: {len(common)}")
N = 20000
for a, b in itertools.combinations(arms, 2):
    for label, f in METRICS:
        diffs = [later(keyed[a][c], f) - later(keyed[b][c], f) for c in common]
        rnd = random.Random(7)
        boots = sorted(sum(rnd.choice(diffs) for _ in diffs) / len(diffs) for _ in range(N))
        p = lambda q: boots[int(q * N)]
        print(f"{a} - {b}, {label}: mean {sum(diffs) / len(diffs):.0f}, "
              f"95% [{p(0.025):.0f}, {p(0.975):.0f}], 99.6% [{p(0.002):.0f}, {p(0.998):.0f}]")
