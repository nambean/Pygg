"""The paired study's results (benchmark/study/README.md, Analysis).

FOLDER holds one folder per person (P01, P02, ...), each with `log.csv` (log-template.csv's columns),
`review-sheet.csv` and `key.json` from study-blind.py, and optionally `pygg-export.json` from study-export.py.

Printed: accepted share per arm and the paired difference P - U with a 95% bootstrap interval resampled by person,
the decision against the plan's rule (no more than 5 points lower, 30+ pairs, 10+ people), median minutes and cost
per arm, defects per task, and from the exports: fallbacks and how many suggested checks were approved.

usage: python -I scripts/study-analyze.py FOLDER [--json OUT]
"""
import argparse
import csv
import json
import random
import statistics
from datetime import datetime
from pathlib import Path

ACCEPTED = {"merge", "merge_after_fix"}


def minutes(row):
    try:
        fmt = "%Y-%m-%d %H:%M"
        return (datetime.strptime(row["finished_at"], fmt) - datetime.strptime(row["started_at"], fmt)).seconds / 60
    except (KeyError, ValueError):
        return None


def person_pairs(folder: Path):
    """Each pair of one person: {pair: {arm: {accepted, defects, minutes, cost}}}."""
    key = json.loads((folder / "key.json").read_text(encoding="utf-8"))
    review = list(csv.DictReader(open(folder / "review-sheet.csv", encoding="utf-8")))
    log = list(csv.DictReader(open(folder / "log.csv", encoding="utf-8")))
    pairs = {}
    for r in review:
        arm = key.get(str(r["pair"]), {}).get(r["diff"])
        if not arm:
            continue
        entry = pairs.setdefault(int(r["pair"]), {}).setdefault(arm, {})
        entry["accepted"] = r["verdict"].strip() in ACCEPTED
        entry["defects"] = int(r["defects"] or 0)
    for row in log:
        arm, pair = row.get("arm", "").strip(), int(row.get("pair") or 0)
        entry = pairs.setdefault(pair, {}).setdefault(arm, {})
        entry["minutes"] = minutes(row)
        entry["cost"] = float(row["cost_usd"]) if row.get("cost_usd") else None
        entry["defects"] = entry.get("defects", 0) + int(row.get("defects_after_a_week") or 0)
        # A task given up, or never reviewed, is not accepted.
        entry.setdefault("accepted", False)
    return {n: p for n, p in pairs.items() if "U" in p and "P" in p}


def main():
    args = argparse.ArgumentParser()
    args.add_argument("folder")
    args.add_argument("--json")
    a = args.parse_args()
    people = {}
    exports = []
    for folder in sorted(p for p in Path(a.folder).iterdir() if p.is_dir()):
        people[folder.name] = person_pairs(folder)
        if (folder / "pygg-export.json").exists():
            exports.append(json.loads((folder / "pygg-export.json").read_text(encoding="utf-8")))
    pairs = [p for ps in people.values() for p in ps.values()]
    if not pairs:
        print("no complete pair yet")
        return
    share = lambda arm, ps: sum(p[arm]["accepted"] for p in ps) / len(ps)
    diff = share("P", pairs) - share("U", pairs)
    rng = random.Random(20261123)
    names = [n for n in people if people[n]]
    boots = []
    for _ in range(4000):
        sample = [p for n in (rng.choice(names) for _ in names) for p in people[n].values()]
        boots.append(share("P", sample) - share("U", sample))
    boots.sort()
    low, high = boots[int(0.025 * len(boots))], boots[int(0.975 * len(boots)) - 1]
    enough = len(pairs) >= 30 and len(names) >= 10
    med = lambda arm, k: statistics.median([p[arm][k] for p in pairs if p[arm].get(k) is not None] or [float("nan")])
    result = {
        "people": len(names), "pairs": len(pairs),
        "acceptedU": round(share("U", pairs), 3), "acceptedP": round(share("P", pairs), 3),
        "differencePoints": round(100 * diff, 1), "interval95Points": [round(100 * low, 1), round(100 * high, 1)],
        "passes": enough and diff >= -0.05, "enoughData": enough,
        "medianMinutes": {"U": med("U", "minutes"), "P": med("P", "minutes")},
        "medianCostUsd": {"U": med("U", "cost"), "P": med("P", "cost")},
        "defectsPerTask": {arm: round(sum(p[arm].get("defects", 0) for p in pairs) / len(pairs), 2) for arm in "UP"},
        "fallbacks": sum(l["fallbacks"] for e in exports for l in e["launches"]),
        "checksSuggested": sum(w["suggested"] for e in exports for w in e["workspaces"]),
        "checksSuggestedApproved": sum(w["suggestedApproved"] for e in exports for w in e["workspaces"]),
        "keptUnchecked": sum(t["keptUnchecked"] for e in exports for t in e["tasks"]),
    }
    print(f"{result['people']} people, {result['pairs']} pairs" + ("" if enough else " (the plan needs 10 people and 30 pairs)"))
    print(f"accepted: usual {100 * result['acceptedU']:.0f}%, Pygg {100 * result['acceptedP']:.0f}%; "
          f"P - U {result['differencePoints']:+.1f} points (95% {result['interval95Points'][0]:+.1f} to {result['interval95Points'][1]:+.1f})")
    print(f"decision: {'passes' if result['passes'] else 'does not pass' if enough else 'not enough data'} (no more than 5 points lower)")
    print(f"median minutes U {result['medianMinutes']['U']}, P {result['medianMinutes']['P']}; "
          f"median cost U {result['medianCostUsd']['U']}, P {result['medianCostUsd']['P']}")
    print(f"defects per task U {result['defectsPerTask']['U']}, P {result['defectsPerTask']['P']}; fallbacks {result['fallbacks']}; "
          f"suggested checks approved {result['checksSuggestedApproved']}/{result['checksSuggested']}; kept unchecked {result['keptUnchecked']}")
    if a.json:
        Path(a.json).write_text(json.dumps(result, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
