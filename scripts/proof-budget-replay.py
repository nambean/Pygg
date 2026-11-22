"""Replays published task-engine runs through Smart Tasks' cost rules (ROADMAP gap items 1 and 3).

For every attempt, in the order the results were written, it computes what the app would have shown before the run
(`tasks::estimate_for`: the lowest and highest of the last 10 recorded costs of the same agent and model) and the cap
it would have offered (`suggested_cap_micro`: half again the highest, rounded up to 5 cents; 50 cents with no history),
then compares with what the attempt cost by the gateway's ledger. For each ladder it applies the engine's task-cap
rules (no new step once the spend reaches the cap; no paid step after one whose cost is unknown) with the task cap set
to the ladder's predicted total, and reports overshoot and solves the cap would have cost.

Order is an approximation: arms ran in parallel, and history here is "written earlier", not "finished earlier".

usage: python scripts/proof-budget-replay.py benchmark/results [--per-project] [--json OUT]
"""
import glob
import json
import math
import statistics
import sys
from collections import defaultdict
from pathlib import Path

RUNS = 10  # tasks::ESTIMATE_RUNS
DEFAULT_CAP = 500_000  # 50 cents with no history


def suggested_cap(high):
    if high is None:
        return DEFAULT_CAP
    return max(1, math.ceil(high * 3 / 2 / 50_000)) * 50_000


def recorded_cost(a):
    """What the app records as the attempt's cost: the ledger sum when every entry is priced, else nothing."""
    if a.get("costStatus") == "estimated":
        return int(a.get("costMicroUsd") or 0)
    return None  # floor or unknown: an unpriced entry makes the app's agent_estimate NULL


def billing(a):
    g = a.get("gateway") or {}
    if a.get("costStatus") == "estimated" and (a.get("costMicroUsd") or 0) == 0:
        return "quota" if g.get("quota") else "free"
    return "money"


def pct(xs, p):
    if not xs:
        return None
    xs = sorted(xs)
    k = (len(xs) - 1) * p
    lo, hi = math.floor(k), math.ceil(k)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def main():
    root = Path(sys.argv[1])
    out_json = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else None
    records = []
    for f in sorted(glob.glob(str(root / "*" / "*.jsonl"))):
        for line in open(f, encoding="utf-8"):
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "attempts" in r and str(r.get("run", "")).startswith("engine:"):
                r["_file"] = Path(f).parent.name + "/" + Path(f).name
                records.append(r)

    # The app keys history by agent and model (every agent here is OpenCode). `--per-project` keys it by the run's
    # project too (its results folder), to see what a per-workspace estimate would change.
    per_project = "--per-project" in sys.argv

    def key(r, model):
        return (r["_file"].split("/")[0], model) if per_project else model

    history = defaultdict(list)  # key -> recorded costs, newest last
    steps = []  # one per money attempt
    ladders = []  # one per record
    interrupted = defaultdict(int)
    for r in records:
        atts = r["attempts"]
        # Estimate the ladder as the app would before it starts: each step's model from its own history.
        models = [a["model"] for a in atts]
        est = []
        for m in models:
            h = history[key(r, m)][-RUNS:]
            est.append((len(h), min(h) if h else None, max(h) if h else None))
        spent, unknown_paid, stopped_by = 0, False, None
        known_total, all_known = 0, True
        for i, a in enumerate(atts):
            cost = recorded_cost(a)
            kind = billing(a)
            if a.get("agentExit") == "timed out":
                interrupted[(kind, a.get("costStatus"))] += 1
            n, lo, hi = est[i]
            if kind == "money":
                cap = suggested_cap(hi)
                steps.append({"model": a["model"], "runs": n, "low": lo, "high": hi, "cap": cap, "cost": cost,
                              "status": a.get("costStatus"), "floor": a.get("costMicroUsd"), "file": r["_file"]})
            if cost is None:
                all_known = False
            else:
                known_total += cost
            if cost is not None:
                history[key(r, a["model"])].append(cost)
        # The ladder's prediction: the sum of each money step's highest recent cost (free and quota steps add 0).
        money_steps = [i for i, a in enumerate(atts) if billing(a) == "money"]
        predicted = None
        if money_steps and all(est[i][2] is not None for i in money_steps):
            predicted = sum(est[i][2] for i in money_steps)
        ladders.append({"run": r["run"], "case": r["caseId"], "file": r["_file"], "passed": r["passed"],
                        "attempts": len(atts), "cost": known_total if all_known else None, "predicted": predicted,
                        "costs": [recorded_cost(a) for a in atts], "kinds": [billing(a) for a in atts],
                        "passed_at": next((i for i, a in enumerate(atts) if a.get("passed")), None),
                        "cascade": ":cascade:" in r["run"]})

    # Steps: estimate error and the per-step cap.
    with_hist = [s for s in steps if s["runs"] >= 3 and s["cost"] is not None]
    inside = sum(1 for s in with_hist if s["low"] <= s["cost"] <= s["high"])
    above = [s for s in with_hist if s["cost"] > s["high"]]
    below = [s for s in with_hist if s["cost"] < s["low"]]
    over_cap = [s for s in with_hist if s["cost"] > s["cap"]]
    ratio = [s["cost"] / s["high"] for s in with_hist if s["high"]]
    mid_err = [abs(s["cost"] - (s["low"] + s["high"]) / 2) / s["cost"] for s in with_hist if s["cost"]]
    no_hist = [s for s in steps if s["runs"] == 0 and s["cost"] is not None]
    unknown_steps = [s for s in steps if s["cost"] is None]

    # Ladders with a prediction and a known cost: the task cap set to the predicted total.
    lad = [l for l in ladders if l["predicted"] is not None and l["predicted"] > 0]
    sim = []
    for l in lad:
        cap = l["predicted"]
        spent, ran, unknown_before, blocked = 0, 0, False, None
        for i, (c, k) in enumerate(zip(l["costs"], l["kinds"])):
            if spent >= cap:
                blocked = "cap"
                break
            if k == "money" and unknown_before:
                blocked = "unknown"
                break
            ran += 1
            if c is None:
                unknown_before = unknown_before or k == "money"
            else:
                spent += c
            if l["passed_at"] == i:
                break
        solved = l["passed_at"] is not None and l["passed_at"] < ran
        sim.append({**l, "cap": cap, "spent": spent, "ran": ran, "blocked": blocked,
                    "solved_under_cap": solved, "over": max(0, spent - cap)})
    overs = [s["over"] / s["cap"] for s in sim if s["over"] > 0]
    lost = [s for s in sim if s["passed"] and not s["solved_under_cap"]]
    blocked_unknown = [s for s in sim if s["blocked"] == "unknown"]
    known_ladders = [l for l in lad if l["cost"] is not None]
    within = sum(1 for l in known_ladders if l["cost"] <= l["predicted"])

    def money(v):
        return "—" if v is None else f"${v / 1e6:.4f}"

    summary = {
        "records": len(records), "money_attempts": len(steps),
        "steps_with_3plus_history": len(with_hist),
        "inside_range": inside, "above_high": len(above), "below_low": len(below),
        "cost_over_offered_cap": len(over_cap),
        "over_cap_excess_median_pct": pct([(s["cost"] - s["cap"]) / s["cap"] * 100 for s in over_cap], 0.5),
        "over_cap_excess_p90_pct": pct([(s["cost"] - s["cap"]) / s["cap"] * 100 for s in over_cap], 0.9),
        "cost_to_high_ratio_median": pct(ratio, 0.5), "cost_to_high_ratio_p90": pct(ratio, 0.9),
        "midpoint_error_median_pct": pct([e * 100 for e in mid_err], 0.5),
        "no_history_steps": len(no_hist), "no_history_over_50c": sum(1 for s in no_hist if s["cost"] > DEFAULT_CAP),
        "unknown_cost_money_attempts": len(unknown_steps),
        "unknown_by_status": dict(defaultdict(int, {k: sum(1 for s in unknown_steps if s["status"] == k)
                                                    for k in ("floor", "unknown")})),
        "ladders_with_prediction": len(lad), "ladders_known_cost": len(known_ladders),
        "ladders_within_predicted": within,
        "ladders_over_task_cap": len(overs), "task_cap_excess_median_pct": pct([o * 100 for o in overs], 0.5),
        "task_cap_excess_p90_pct": pct([o * 100 for o in overs], 0.9),
        "task_cap_excess_max_pct": max([o * 100 for o in overs], default=None),
        "solves_lost_to_task_cap": len(lost), "solves_total": sum(1 for s in sim if s["passed"]),
        "blocked_by_unknown_cost": len(blocked_unknown),
        "cascades_with_prediction": len([s for s in sim if s["cascade"]]),
        "cascades_climbed": len([s for s in sim if s["cascade"] and s["attempts"] > 1]),
        "cascades_over_task_cap": len([s for s in sim if s["cascade"] and s["over"] > 0]),
        "cascades_stopped_by_cap": len([s for s in sim if s["cascade"] and s["blocked"]]),
        "cascade_solves_lost_to_cap": len([s for s in lost if s["cascade"]]),
        "interrupted_attempts": {f"{k[0]}/{k[1]}": v for k, v in sorted(interrupted.items())},
    }
    for k, v in summary.items():
        print(f"{k:34} {v}")
    print("\nWorst steps over the offered cap:")
    for s in sorted(over_cap, key=lambda s: -(s["cost"] / s["cap"]))[:8]:
        print(f"  {s['model'][:50]:50} runs={s['runs']:>2} high={money(s['high'])} cap={money(s['cap'])} "
              f"cost={money(s['cost'])} ({s['file']})")
    if out_json:
        Path(out_json).write_text(json.dumps({"summary": summary, "ladders": sim}, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
